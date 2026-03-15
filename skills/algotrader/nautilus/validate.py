"""Validate a parameter set by running a single NautilusTrader backtest.

Takes params from a JSON file (output of evolve.py) or CLI overrides,
runs the Qullamaggie strategy, and prints detailed results.

Supports resampling 5m bars to 1h for out-of-sample validation on
a different timeframe than the optimizer used.

Usage:
    # Run best params from optimizer output on daily bars:
    python -m nautilus.validate --params results.json

    # Run on hourly bars (resampled from 5m data):
    python -m nautilus.validate --params results.json --profile crypto_5m --resample 1h

    # Override specific params:
    python -m nautilus.validate --profile crypto_5m --resample 1h \
        --set rs_pct=0.50 vol_ratio=2.0 min_prior_move=0.03
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

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
from nautilus.strategy import QullamaggieBreakout, QullamaggieConfig
from trading_config import load_trading_config


DEFAULT_TICKERS = [
    "BTCUSDT", "SOLUSDT", "AVAXUSDT", "NEARUSDT", "APTUSDT",
    "INJUSDT", "SUIUSDT", "FETUSDT", "RNDRUSDT", "SEIUSDT", "TIAUSDT",
]


def resample_to_hourly(df_5m: pd.DataFrame) -> pd.DataFrame:
    """Resample 5-minute OHLCV bars to 1-hour bars."""
    return df_5m.resample("1h").agg({
        "open": "first",
        "high": "max",
        "low": "min",
        "close": "last",
        "volume": "sum",
    }).dropna()


def run_validation(
    data_dir: Path,
    tickers: list[str],
    params: dict,
    resample: str | None = None,
    max_bars: int | None = None,
    start: str | None = None,
    end: str | None = None,
    log_level: str = "WARNING",
) -> dict:
    """Run a single backtest and return detailed results."""
    init_cash = params.get("init_cash", 1_000_000.0)

    # Determine data source and bar spec
    if resample == "1h":
        source_tf = "5m"
        bar_spec = "1-HOUR-LAST"
    else:
        source_tf = "1d"
        bar_spec = "1-DAY-LAST"

    engine = BacktestEngine(
        config=BacktestEngineConfig(
            trader_id=TraderId("VALIDATE-001"),
            logging=LoggingConfig(log_level=log_level),
        ),
    )

    engine.add_venue(
        venue=BINANCE_VENUE,
        oms_type=OmsType.NETTING,
        account_type=AccountType.CASH,
        base_currency=None,
        starting_balances=[Money(init_cash, USDT)],
        bar_execution=True,
    )

    instrument_ids = []
    total_bars = 0

    for ticker in tickers:
        df = load_ticker_csv(data_dir, ticker, source_tf)
        if df is None or df.empty:
            print(f"  {ticker}: no {source_tf} data, skipping")
            continue

        if resample == "1h":
            df = resample_to_hourly(df)

        if start:
            df = df[df.index >= pd.Timestamp(start, tz="UTC")]
        if end:
            df = df[df.index <= pd.Timestamp(end, tz="UTC")]

        if df.empty:
            continue

        if max_bars and len(df) > max_bars:
            df = df.iloc[-max_bars:]

        instrument = make_instrument(ticker)
        engine.add_instrument(instrument)

        bt = BarType.from_str(f"{ticker}.BINANCE-{bar_spec}-EXTERNAL")
        wrangler = BarDataWrangler(bt, instrument)
        bars = wrangler.process(df)
        engine.add_data(bars, sort=False)

        instrument_ids.append(f"{ticker}.BINANCE")
        total_bars += len(bars)
        print(f"  {ticker}: {len(bars):,} bars")

    if not instrument_ids:
        print("Error: no data loaded", file=sys.stderr)
        return {}

    engine.sort_data()

    config = QullamaggieConfig(
        instrument_ids=instrument_ids,
        bar_spec=bar_spec,
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
    )

    strategy = QullamaggieBreakout(config=config)
    engine.add_strategy(strategy)

    print(f"\nRunning backtest: {len(instrument_ids)} instruments, {total_bars:,} bars...")
    t0 = time.time()
    engine.run()
    elapsed = time.time() - t0
    print(f"Completed in {elapsed:.1f}s ({total_bars / elapsed:,.0f} bars/sec)")

    # Extract results
    result = engine.get_result()
    pnl_stats = result.stats_pnls.get("USDT", {})
    ret_stats = result.stats_returns or {}

    try:
        account = engine.kernel.portfolio.account(BINANCE_VENUE)
        final_equity = float(account.balance_total(USDT))
    except Exception:
        final_equity = init_cash

    total_return = (final_equity - init_cash) / init_cash if init_cash > 0 else 0.0

    # Max drawdown from returns series
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

    # CAGR
    if result.backtest_start and result.backtest_end:
        seconds = (result.backtest_end - result.backtest_start) / 1e9
        years = seconds / (365.25 * 86400)
    else:
        years = 1.0

    if years > 0 and total_return > -1.0:
        cagr = (1.0 + total_return) ** (1.0 / years) - 1.0
    else:
        cagr = 0.0

    report = {
        "total_trades": result.total_positions,
        "win_rate": _f(pnl_stats.get("Win Rate", 0.0)),
        "profit_factor": _f(ret_stats.get("Profit Factor", 0.0)),
        "avg_win": _f(pnl_stats.get("Avg Winner", 0.0)),
        "avg_loss": _f(pnl_stats.get("Avg Loser", 0.0)),
        "sharpe": _f(ret_stats.get("Sharpe Ratio (252 days)", 0.0)),
        "sortino": _f(ret_stats.get("Sortino Ratio (252 days)", 0.0)),
        "total_return": total_return,
        "cagr": cagr,
        "max_drawdown": max_drawdown,
        "init_cash": init_cash,
        "final_equity": final_equity,
        "elapsed_s": elapsed,
        "total_bars": total_bars,
        "instruments": len(instrument_ids),
        "years": years,
    }

    # Print full reports
    print("\n--- Account Report ---")
    print(engine.trader.generate_account_report(BINANCE_VENUE))
    print("\n--- Positions ---")
    print(engine.trader.generate_positions_report())

    engine.dispose()
    return report


def _f(v) -> float:
    if v is None:
        return 0.0
    try:
        return float(v)
    except (TypeError, ValueError):
        return 0.0


def main():
    parser = argparse.ArgumentParser(description="Validate params on full dataset")
    parser.add_argument("--profile", default="crypto_daily", help="Configured dataset profile")
    parser.add_argument("--params", type=Path, help="JSON file with best_params (from evolve output)")
    parser.add_argument("--tickers", help="Comma-separated tickers")
    parser.add_argument("--resample", choices=["1h"], help="Resample 5m data to 1h bars")
    parser.add_argument("--max-bars", type=int, help="Limit bars per ticker (use tail)")
    parser.add_argument("--init-cash", type=float, default=1_000_000.0)
    parser.add_argument("--log-level", default="WARNING")
    parser.add_argument("--set", nargs="*", help="Override params: key=value pairs")
    args = parser.parse_args()
    cfg = load_trading_config()
    profile = cfg.profile(args.profile)

    # Load base params
    if args.params:
        with open(args.params) as f:
            data = json.load(f)
        params = data.get("best_params", data)
    else:
        params = {}

    params["init_cash"] = args.init_cash

    # Apply CLI overrides
    if args.set:
        for kv in args.set:
            k, v = kv.split("=", 1)
            try:
                params[k] = int(v)
            except ValueError:
                try:
                    params[k] = float(v)
                except ValueError:
                    params[k] = v

    # Resolve tickers
    if args.tickers:
        tickers = [s.strip().upper() for s in args.tickers.split(",")]
    else:
        tickers = list(DEFAULT_TICKERS)
    if "BTCUSDT" not in tickers:
        tickers.insert(0, "BTCUSDT")

    print("=== Validation Run ===")
    print(f"Resample: {args.resample or 'none (daily)'}")
    print(f"Max bars: {args.max_bars or 'all'}")
    print(f"Init cash: ${args.init_cash:,.0f}")
    print(f"\nParams:")
    for k in sorted(params):
        if k not in ("setup", "bars_per_day", "min_price", "min_vol", "min_adv",
                      "slippage_k", "max_sma_ext", "split_frac", "min_consol_days"):
            print(f"  {k}: {params[k]}")
    print(f"\nLoading data...")

    report = run_validation(
        data_dir=cfg.datasets[profile.dataset],
        tickers=tickers,
        params=params,
        resample=args.resample,
        max_bars=args.max_bars,
        log_level=args.log_level,
    )

    if report:
        print("\n=== Summary ===")
        print(f"  Trades:        {report['total_trades']}")
        print(f"  Win Rate:      {report['win_rate']:.1%}")
        print(f"  Profit Factor: {report['profit_factor']:.2f}")
        print(f"  Avg Win:       ${report['avg_win']:,.2f}")
        print(f"  Avg Loss:      ${report['avg_loss']:,.2f}")
        print(f"  Sharpe:        {report['sharpe']:.2f}")
        print(f"  Sortino:       {report['sortino']:.2f}")
        print(f"  Total Return:  {report['total_return']:.2%}")
        print(f"  CAGR:          {report['cagr']:.2%}")
        print(f"  Max Drawdown:  {report['max_drawdown']:.2%}")
        print(f"  Final Equity:  ${report['final_equity']:,.2f}")
        print(f"  Duration:      {report['years']:.1f} years")


if __name__ == "__main__":
    main()
