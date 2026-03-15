"""Evolutionary optimizer for the NautilusTrader Qullamaggie strategy.

Uses multiprocessing to run backtests in parallel. Each worker:
  1. Loads CSV data and wrangles Bar objects once
  2. Pre-computes all indicator values as numpy arrays (like the Rust engine)
  3. Creates a single BacktestEngine, reused across runs via reset()

Reuses the evolutionary loop, fitness, and selection/mutation logic from
scripts/evolve.py — only the transport layer is NautilusTrader-specific.

Usage:
    # From skills/algotrader/:
    python -m nautilus.evolve
    python -m nautilus.evolve --profile crypto_5m --tickers BTCUSDT,SOLUSDT --workers 4
    python -m nautilus.evolve --profile crypto_daily --generations 50 --pop-size 100
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from multiprocessing import Pool
from pathlib import Path

import numpy as np
import pandas as pd

_project_root = str(Path(__file__).resolve().parent.parent)
if _project_root not in sys.path:
    sys.path.insert(0, _project_root)

from nautilus_trader.adapters.binance import BINANCE_VENUE
from nautilus_trader.backtest.engine import BacktestEngine
from nautilus_trader.config import BacktestEngineConfig, LoggingConfig
from nautilus_trader.model.currencies import USDT
from nautilus_trader.model.data import BarType
from nautilus_trader.model.enums import AccountType, OmsType
from nautilus_trader.model.identifiers import TraderId
from nautilus_trader.model.objects import Money
from nautilus_trader.persistence.wranglers import BarDataWrangler

from scripts.nautilus_backtest import load_ticker_csv, make_instrument
from nautilus.strategy import PrecomputedBreakout, PrecomputedConfig
from scripts.evolve import evolve
from trading_config import load_trading_config

# ---------------------------------------------------------------------------
# Indicator pre-computation (vectorized, runs once per worker)
# ---------------------------------------------------------------------------

def _wilder_atr(df: pd.DataFrame, period: int = 14) -> np.ndarray:
    """Wilder's ATR (alpha=1/n) matching our Rust engine."""
    high = df["high"].values
    low = df["low"].values
    close = df["close"].values
    n = len(close)
    tr = np.empty(n, dtype=np.float64)
    tr[0] = high[0] - low[0]
    for i in range(1, n):
        tr[i] = max(high[i] - low[i], abs(high[i] - close[i - 1]), abs(low[i] - close[i - 1]))
    atr = np.empty(n, dtype=np.float64)
    atr[:period] = np.nan
    atr[period - 1] = np.mean(tr[:period])
    alpha = 1.0 / period
    for i in range(period, n):
        atr[i] = atr[i - 1] * (1 - alpha) + tr[i] * alpha
    return atr


def _vcp_detect(highs: np.ndarray, lows: np.ndarray, closes: np.ndarray, volumes: np.ndarray,
                lookback: int = 60, swing_order: int = 5) -> tuple[np.ndarray, ...]:
    """Vectorized VCP detection returning (num_contractions, last_contraction_pct, tightening_ratio, vol_trend)."""
    n = len(closes)
    out_nc = np.zeros(n, dtype=np.float64)
    out_lcp = np.zeros(n, dtype=np.float64)
    out_tr = np.ones(n, dtype=np.float64)
    out_vt = np.ones(n, dtype=np.float64)

    for end in range(lookback, n + 1):
        start = end - lookback
        h = highs[start:end]
        l = lows[start:end]
        v = volumes[start:end]
        nn = len(h)
        order = swing_order

        swing_highs = []
        swing_lows = []
        for i in range(order, nn - order):
            hi, lo = h[i], l[i]
            is_sh = is_sl = True
            for j in range(i - order, i + order + 1):
                if h[j] > hi:
                    is_sh = False
                if l[j] < lo:
                    is_sl = False
                if not is_sh and not is_sl:
                    break
            if is_sh:
                swing_highs.append((i, hi))
            if is_sl:
                swing_lows.append((i, lo))

        contractions = []
        sl_idx = 0
        for sh_bar, sh_price in swing_highs:
            while sl_idx < len(swing_lows) and swing_lows[sl_idx][0] <= sh_bar:
                sl_idx += 1
            if sl_idx < len(swing_lows) and sh_price > 0.0:
                sl_bar, sl_price = swing_lows[sl_idx]
                contractions.append((sh_price, sl_price, sh_bar, sl_bar))

        idx = end - 1
        if not contractions:
            continue

        ranges = [(c[0] - c[1]) / c[0] for c in contractions]
        progressive = 1
        for i in range(1, len(ranges)):
            if ranges[i] < ranges[i - 1]:
                progressive += 1
            else:
                break

        out_nc[idx] = progressive
        out_lcp[idx] = ranges[-1]
        if len(ranges) >= 2 and ranges[0] > 0.0:
            out_tr[idx] = ranges[-1] / ranges[0]

        def avg_vol(arr, s, e):
            sl = arr[s:min(e + 1, nn)]
            return np.mean(sl) if len(sl) > 0 else 0.0

        vf = avg_vol(v, contractions[0][2], contractions[0][3])
        vl = avg_vol(v, contractions[-1][2], contractions[-1][3])
        out_vt[idx] = (vl / vf) if vf > 0.0 else 1.0

    return out_nc, out_lcp, out_tr, out_vt


def _flag_detect(highs: np.ndarray, lows: np.ndarray, closes: np.ndarray, volumes: np.ndarray,
                 lookback: int = 60, swing_order: int = 5,
                 max_flag_bars: int = 25, max_pole_bars: int = 20) -> tuple[np.ndarray, ...]:
    """Vectorized flag detection returning (pole_pct, retrace_pct, flag_days, vol_ratio)."""
    n = len(closes)
    out_pp = np.zeros(n, dtype=np.float64)
    out_rp = np.zeros(n, dtype=np.float64)
    out_fd = np.zeros(n, dtype=np.float64)
    out_vr = np.ones(n, dtype=np.float64)

    for end in range(lookback, n + 1):
        start = end - lookback
        h = highs[start:end]
        l = lows[start:end]
        c = closes[start:end]
        v = volumes[start:end]
        nn = len(h)
        order = swing_order
        current_idx = nn - 1

        swing_highs = []
        swing_lows = []
        for i in range(order, nn - order):
            hi, lo = h[i], l[i]
            is_sh = is_sl = True
            for j in range(i - order, i + order + 1):
                if h[j] > hi:
                    is_sh = False
                if l[j] < lo:
                    is_sl = False
                if not is_sh and not is_sl:
                    break
            if is_sh:
                swing_highs.append((i, hi))
            if is_sl:
                swing_lows.append((i, lo))

        best_gain = 0.0
        best_pole = None
        for sh_idx, sh_price in swing_highs:
            bars_since = current_idx - sh_idx
            if bars_since < 0 or bars_since > max_flag_bars:
                continue
            for sl_idx, sl_price in reversed(swing_lows):
                if sl_idx >= sh_idx:
                    continue
                if sh_idx - sl_idx > max_pole_bars:
                    break
                if sl_price <= 0.0:
                    continue
                gain = (sh_price - sl_price) / sl_price
                if gain >= 0.20 and gain > best_gain:
                    best_gain = gain
                    best_pole = (sl_idx, sl_price, sh_idx, sh_price)
                break

        idx = end - 1
        if best_pole is None:
            continue

        sl_i, sl_p, sh_i, sh_p = best_pole
        move = sh_p - sl_p
        out_pp[idx] = (sh_p - sl_p) / sl_p
        out_fd[idx] = current_idx - sh_i
        out_rp[idx] = (sh_p - c[current_idx]) / move if move > 0 else 0.0

        def avg_vol(arr, s, e):
            sl = arr[s:min(e + 1, nn)]
            return np.mean(sl) if len(sl) > 0 else 0.0

        vp = avg_vol(v, sl_i, sh_i)
        vf = avg_vol(v, sh_i, current_idx)
        out_vr[idx] = (vf / vp) if vp > 0.0 else 1.0

    return out_pp, out_rp, out_fd, out_vr


def _precompute_indicators(df: pd.DataFrame, bars_per_day: int = 1, high_lookback: int = 252) -> dict[str, np.ndarray]:
    """Compute all indicator arrays from an OHLCV DataFrame.

    All periods are in "days" and scaled by bars_per_day internally.
    Keys include the bar-scaled period (e.g. ret_63 for daily, ret_18144 for 5m).
    """
    bpd = bars_per_day
    close = df["close"].values.astype(np.float64)
    high = df["high"].values.astype(np.float64)
    low = df["low"].values.astype(np.float64)
    volume = df["volume"].values.astype(np.float64)

    sma10 = pd.Series(close).rolling(10 * bpd).mean().values
    sma20 = pd.Series(close).rolling(20 * bpd).mean().values
    atr14 = _wilder_atr(df, 14 * bpd)
    vol_sma20 = pd.Series(volume).rolling(20 * bpd).mean().values
    rolling_high = pd.Series(high).rolling(high_lookback * bpd).max().values

    close_s = pd.Series(close)
    ret_7 = close_s.pct_change(7 * bpd).values
    ret_14 = close_s.pct_change(14 * bpd).values
    ret_21 = (close_s / close_s.shift(21 * bpd) - 1.0).values
    ret_63 = (close_s / close_s.shift(63 * bpd) - 1.0).values
    ret_126 = (close_s / close_s.shift(126 * bpd) - 1.0).values

    ema10 = close_s.ewm(span=10 * bpd, adjust=False).mean().values
    ema20 = close_s.ewm(span=20 * bpd, adjust=False).mean().values

    # Pattern detectors: lookback stays small (O(lookback²) per bar).
    # On daily: 60 bars = 60 days. On 5m: 60 bars = 5 hours.
    vcp_nc, vcp_lcp, vcp_tr, vcp_vt = _vcp_detect(
        high, low, close, volume, lookback=60,
    )
    flag_pp, flag_rp, flag_fd, flag_vr = _flag_detect(
        high, low, close, volume, lookback=60,
        max_flag_bars=25, max_pole_bars=20,
    )

    return {
        "sma10": sma10, "sma20": sma20, "atr14": atr14,
        "vol_sma20": vol_sma20, "rolling_high": rolling_high,
        f"ret_{7 * bpd}": ret_7, f"ret_{14 * bpd}": ret_14,
        f"ret_{21 * bpd}": ret_21, f"ret_{63 * bpd}": ret_63, f"ret_{126 * bpd}": ret_126,
        "ema10": ema10, "ema20": ema20,
        "vcp_nc": vcp_nc, "vcp_lcp": vcp_lcp, "vcp_tr": vcp_tr, "vcp_vt": vcp_vt,
        "flag_pp": flag_pp, "flag_rp": flag_rp, "flag_fd": flag_fd, "flag_vr": flag_vr,
    }


# ---------------------------------------------------------------------------
# Worker globals — initialized once per worker process
# ---------------------------------------------------------------------------

_w_instruments: dict = {}
_w_bars: dict = {}
_w_bar_spec: str = ""
_w_tickers: list[str] = []
_w_engine: BacktestEngine | None = None
_w_indicators: dict = {}  # {ticker: {name: np.ndarray}}
_w_init_cash: float = 1_000_000.0
_w_bars_per_day: int = 1
_w_high_lookback: int = 252


def _init_worker(
    data_dir_str: str,
    tickers: list[str],
    bar_spec: str,
    timeframe: str,
    resample: str | None = None,
    start: str | None = None,
    end: str | None = None,
    bars_per_day: int = 1,
    high_lookback: int = 252,
) -> None:
    """Pool initializer: load data, pre-compute indicators, create engine once."""
    global _w_instruments, _w_bars, _w_bar_spec, _w_tickers, _w_engine, _w_indicators, _w_bars_per_day, _w_high_lookback

    _w_bar_spec = bar_spec
    _w_tickers = tickers
    _w_bars_per_day = bars_per_day
    _w_high_lookback = high_lookback
    _w_instruments.clear()
    _w_bars.clear()
    _w_indicators.clear()
    data_dir = Path(data_dir_str)

    source_tf = "5m" if resample == "1h" else timeframe

    for ticker in tickers:
        df = load_ticker_csv(data_dir, ticker, source_tf)
        if df is None or df.empty:
            continue

        if resample == "1h":
            df = df.resample("1h").agg({
                "open": "first", "high": "max", "low": "min",
                "close": "last", "volume": "sum",
            }).dropna()

        if start:
            df = df[df.index >= pd.Timestamp(start, tz="UTC")]
        if end:
            df = df[df.index <= pd.Timestamp(end, tz="UTC")]

        if df.empty:
            continue

        _w_indicators[ticker] = _precompute_indicators(df, bars_per_day=bars_per_day, high_lookback=high_lookback)

        instrument = make_instrument(ticker)
        bt = BarType.from_str(f"{ticker}.BINANCE-{bar_spec}-EXTERNAL")
        wrangler = BarDataWrangler(bt, instrument)
        bars = wrangler.process(df)
        _w_instruments[ticker] = instrument
        _w_bars[ticker] = bars

    if not _w_instruments:
        return

    _w_engine = BacktestEngine(
        config=BacktestEngineConfig(
            trader_id=TraderId("EVOLVE-001"),
            logging=LoggingConfig(bypass_logging=True),
        ),
    )

    _w_engine.add_venue(
        venue=BINANCE_VENUE,
        oms_type=OmsType.NETTING,
        account_type=AccountType.CASH,
        base_currency=None,
        starting_balances=[Money(_w_init_cash, USDT)],
        bar_execution=True,
    )

    for ticker in _w_tickers:
        if ticker not in _w_instruments:
            continue
        _w_engine.add_instrument(_w_instruments[ticker])
        _w_engine.add_data(_w_bars[ticker], sort=False)

    _w_engine.sort_data()


def _run_one(params: dict) -> dict:
    """Run a single backtest reusing the worker's engine."""
    if _w_engine is None:
        return _empty_report(params, _w_init_cash)

    engine = _w_engine
    engine.reset()
    engine.clear_strategies()

    instrument_ids = [f"{t}.BINANCE" for t in _w_tickers if t in _w_instruments]
    if not instrument_ids:
        return _empty_report(params, _w_init_cash)

    config = PrecomputedConfig(
        instrument_ids=instrument_ids,
        bar_spec=_w_bar_spec,
        rs_pct=params.get("rs_pct", 0.30),
        max_dist_52w=params.get("max_dist_52w", 0.25),
        min_prior_move=params.get("min_prior_move", 0.30),
        min_adr_pct=params.get("min_adr_pct", 0.03),
        vol_spike=params.get("vol_spike", params.get("vol_ratio", 1.5)),
        max_range_pct=params.get("max_range_pct", 0.15),
        risk_pct=params.get("risk_pct", 0.005),
        max_pos_pct=params.get("max_pos_pct", 0.20),
        partial_bars=params.get("partial_bars", 5),
        trail_period=params.get("trail_period", 10),
        split_frac=params.get("split_frac", 0.50),
        max_hold_bars=params.get("max_hold_bars", 0),
        flag_min_pole=params.get("flag_min_pole", 0.20),
        flag_max_retrace=params.get("flag_max_retrace", 0.50),
        flag_min_days=params.get("flag_min_days", 5),
        flag_max_days=params.get("flag_max_days", 25),
        rs_lookback=params.get("rs_lookback", 63),
        bars_per_day=_w_bars_per_day,
        high_lookback=_w_high_lookback,
    )

    strategy = PrecomputedBreakout(config=config, indicators=_w_indicators)
    engine.add_strategy(strategy)

    try:
        engine.run()
    except Exception:
        return _empty_report(params, _w_init_cash)

    return _extract_report(engine, params, _w_init_cash)


def _extract_report(engine: BacktestEngine, params: dict, init_cash: float) -> dict:
    """Pull metrics from the engine after a run."""
    result = engine.get_result()

    pnl_stats = result.stats_pnls.get("USDT", {})
    ret_stats = result.stats_returns or {}

    try:
        account = engine.kernel.portfolio.account(BINANCE_VENUE)
        final_equity = float(account.balance_total(USDT))
    except Exception:
        final_equity = init_cash

    total_return = (final_equity - init_cash) / init_cash if init_cash > 0 else 0.0

    if result.backtest_start and result.backtest_end:
        seconds = (result.backtest_end - result.backtest_start) / 1e9
        years = seconds / (365.25 * 86400)
    else:
        years = 1.0

    if years > 0 and total_return > -1.0:
        cagr = (1.0 + total_return) ** (1.0 / years) - 1.0
    else:
        cagr = 0.0

    try:
        returns = engine.kernel.portfolio.analyzer._returns
        if len(returns) > 0:
            cumulative = (1.0 + returns).cumprod()
            running_max = cumulative.cummax()
            drawdowns = (cumulative - running_max) / running_max
            max_drawdown = abs(float(drawdowns.min()))
        else:
            max_drawdown = 0.0
    except Exception:
        max_drawdown = 0.0

    return {
        "total_trades": result.total_positions,
        "win_rate": _float(pnl_stats.get("Win Rate", 0.0)),
        "profit_factor": _float(ret_stats.get("Profit Factor", 0.0)),
        "avg_win": _float(pnl_stats.get("Avg Winner", 0.0)),
        "avg_loss": _float(pnl_stats.get("Avg Loser", 0.0)),
        "sharpe": _float(ret_stats.get("Sharpe Ratio (252 days)", 0.0)),
        "sortino": _float(ret_stats.get("Sortino Ratio (252 days)", 0.0)),
        "total_return": total_return,
        "cagr": cagr,
        "max_drawdown": max_drawdown,
        "init_cash": init_cash,
        "final_equity": final_equity,
        "params": params,
    }


def _float(v) -> float:
    if v is None:
        return 0.0
    try:
        return float(v)
    except (TypeError, ValueError):
        return 0.0


def _empty_report(params: dict, init_cash: float) -> dict:
    return {
        "total_trades": 0,
        "win_rate": 0.0,
        "profit_factor": 0.0,
        "avg_win": 0.0,
        "avg_loss": 0.0,
        "sharpe": 0.0,
        "sortino": 0.0,
        "total_return": 0.0,
        "cagr": 0.0,
        "max_drawdown": 0.0,
        "init_cash": init_cash,
        "final_equity": init_cash,
        "params": params,
    }


# ---------------------------------------------------------------------------
# NautilusTransport — drop-in replacement for ServerTransport/SubprocessTransport
# ---------------------------------------------------------------------------

class NautilusTransport:
    """Run NautilusTrader backtests in a multiprocessing pool."""

    def __init__(
        self,
        data_dir: Path,
        tickers: list[str],
        bar_spec: str = "1-DAY-LAST",
        timeframe: str = "1d",
        resample: str | None = None,
        start: str | None = None,
        end: str | None = None,
        n_workers: int | None = None,
        bars_per_day: int = 1,
        high_lookback: int = 252,
    ):
        self.data_dir = data_dir.resolve()
        self.tickers = tickers
        self.bar_spec = bar_spec
        self.timeframe = timeframe
        self.resample = resample
        self.start = start
        self.end = end
        self.n_workers = n_workers or os.cpu_count() or 4
        self.bars_per_day = bars_per_day
        self.high_lookback = high_lookback
        self.pool: Pool | None = None

    def connect(self) -> None:
        self.pool = Pool(
            processes=self.n_workers,
            initializer=_init_worker,
            initargs=(
                str(self.data_dir), self.tickers, self.bar_spec,
                self.timeframe, self.resample, self.start, self.end,
                self.bars_per_day, self.high_lookback,
            ),
        )

    def close(self) -> None:
        if self.pool:
            self.pool.terminate()
            self.pool.join()
            self.pool = None

    def run_batch(self, params_list: list[dict]) -> list[dict]:
        if not self.pool:
            self.connect()
        results = self.pool.map(_run_one, params_list)
        return results


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

DEFAULT_TICKERS = [
    "BTCUSDT", "SOLUSDT", "AVAXUSDT", "NEARUSDT", "APTUSDT",
    "INJUSDT", "SUIUSDT", "FETUSDT", "RNDRUSDT", "SEIUSDT", "TIAUSDT",
]


def main():
    parser = argparse.ArgumentParser(description="Evolutionary optimizer (NautilusTrader)")
    parser.add_argument("--profile", default="crypto_daily", help="Configured dataset profile")
    parser.add_argument("--tickers", help="Comma-separated tickers (always includes BTCUSDT)")
    parser.add_argument("--bar-spec", default=None, help="Override bar spec (e.g. 1-HOUR-LAST)")
    parser.add_argument("--resample", choices=["1h"], help="Resample 5m data to 1h bars")
    parser.add_argument("--start", help="Start date filter (ISO, e.g. 2020-01-01)")
    parser.add_argument("--end", help="End date filter (ISO, e.g. 2021-12-31)")
    parser.add_argument("--workers", type=int, default=os.cpu_count())
    parser.add_argument("--generations", type=int, default=50)
    parser.add_argument("--pop-size", type=int, default=100)
    parser.add_argument("--elite-frac", type=float, default=0.25)
    parser.add_argument("--fitness", default="composite")
    parser.add_argument("--crypto", action="store_true", default=True)
    parser.add_argument("--high-lookback", type=int, default=252, help="Rolling high lookback in days (252=1yr, 30=1mo)")
    parser.add_argument("--output", default=None, help="Output JSON path")
    args = parser.parse_args()

    cfg = load_trading_config()
    profile = cfg.profile(args.profile)
    data_dir = cfg.datasets[profile.dataset]
    timeframe = profile.timeframe

    if args.tickers:
        tickers = [s.strip().upper() for s in args.tickers.split(",")]
    else:
        tickers = list(DEFAULT_TICKERS)
    if "BTCUSDT" not in tickers:
        tickers.insert(0, "BTCUSDT")

    if args.bar_spec:
        bar_spec = args.bar_spec
    elif args.resample == "1h":
        bar_spec = "1-HOUR-LAST"
    else:
        bar_spec = "1-DAY-LAST" if timeframe == "1d" else "5-MINUTE-LAST"

    print(f"NautilusTrader evolutionary optimizer", file=sys.stderr)
    print(f"  Data: {data_dir}", file=sys.stderr)
    print(f"  Tickers: {len(tickers)} ({', '.join(tickers[:5])}...)", file=sys.stderr)
    print(f"  Bar spec: {bar_spec}", file=sys.stderr)
    # Compute bars_per_day from the timeframe
    if args.resample == "1h":
        bars_per_day = 24
    elif timeframe == "5m":
        bars_per_day = 288
    else:
        bars_per_day = 1

    if args.resample:
        print(f"  Resample: 5m → {args.resample}", file=sys.stderr)
    if args.start or args.end:
        print(f"  Date range: {args.start or 'start'} → {args.end or 'end'}", file=sys.stderr)
    print(f"  Workers: {args.workers}, bars_per_day: {bars_per_day}", file=sys.stderr)
    print(f"  Generations: {args.generations}, Pop: {args.pop_size}, Fitness: {args.fitness}", file=sys.stderr)

    transport = NautilusTransport(
        data_dir=data_dir,
        tickers=tickers,
        bar_spec=bar_spec,
        timeframe=args.timeframe,
        resample=args.resample,
        start=args.start,
        end=args.end,
        n_workers=args.workers,
        bars_per_day=bars_per_day,
        high_lookback=args.high_lookback,
    )

    transport.connect()
    t0 = time.time()
    try:
        best_params, best_report = evolve(
            transport=transport,
            generations=args.generations,
            pop_size=args.pop_size,
            elite_frac=args.elite_frac,
            metric=args.fitness,
            crypto=args.crypto,
        )
    finally:
        transport.close()

    total = time.time() - t0
    total_runs = args.generations * args.pop_size
    print(f"\nDone in {total:.1f}s ({total_runs} backtests, {total / total_runs * 1000:.0f}ms/run avg)", file=sys.stderr)

    result = {
        "best_params": best_params,
        "best_report": best_report,
    }

    if args.output:
        Path(args.output).write_text(json.dumps(result, indent=2))
        print(f"Saved to {args.output}", file=sys.stderr)
    else:
        print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
