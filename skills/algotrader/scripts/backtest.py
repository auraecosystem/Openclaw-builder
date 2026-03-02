#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "vectorbt>=0.25",
#     "pandas>=2.0",
#     "numpy",
#     "pyarrow",
#     "numba",
# ]
# ///
"""
Qullamaggie swing trading backtester.

Loads all 11k+ tickers into RAM as wide DataFrames, computes indicators once,
then runs vectorbt Portfolio.from_signals() for each setup.

Usage:
    uv run backtest.py --data-dir <path>/data
    uv run backtest.py --data-dir <path>/data --setup breakout --start 2020-01-01
    uv run backtest.py --data-dir <path>/data --setup all --output /tmp/results.json
"""

import argparse
import json
import os
import sys
import time
from pathlib import Path

# Allow imports from lib/ (sibling of scripts/)
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "lib"))

import numpy as np
import pandas as pd
import vectorbt as vbt

import indicators as ind
import universe as uni
import patterns as pat
import signals as sig
import risk
import cache as _cache
from portfolio import PortfolioState, Position, generate_report, save_report


def _log(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr)


def _trades_to_positions(
    trades_df: pd.DataFrame,
    setup: str,
) -> list[Position]:
    """Convert vectorbt trades records to Position objects.

    vectorbt records_readable columns:
      Column, Size, Entry Timestamp, Avg Entry Price, Exit Timestamp,
      Avg Exit Price, PnL, Return, Direction, Status
    """
    positions = []
    for _, row in trades_df.iterrows():
        ticker = str(row.get("Column", ""))
        direction = "short" if str(row.get("Direction", "Long")) == "Short" else "long"
        pnl = float(row.get("PnL", 0))
        entry_price = float(row.get("Avg Entry Price", 0))
        exit_price = float(row.get("Avg Exit Price", 0))
        shares = int(abs(row.get("Size", 0)))
        entry_ts = row.get("Entry Timestamp")
        exit_ts = row.get("Exit Timestamp")
        entry_date = str(entry_ts) if entry_ts is not None else ""
        exit_date = str(exit_ts) if exit_ts is not None else ""
        hold_days = 0
        if entry_ts is not None and exit_ts is not None:
            try:
                hold_days = (pd.Timestamp(exit_ts) - pd.Timestamp(entry_ts)).days
            except Exception:
                pass
        stop_price = entry_price * 0.95  # approximate; real stop tracked separately

        pos = Position(
            ticker=ticker,
            setup=setup,
            entry_date=entry_date,
            entry_price=entry_price,
            shares=shares,
            stop_price=stop_price,
            direction=direction,
            exit_date=exit_date,
            exit_price=exit_price,
            pnl=pnl,
            pnl_pct=pnl / (entry_price * shares) if entry_price * shares != 0 else 0.0,
            hold_days=hold_days,
        )
        positions.append(pos)
    return positions


def _run_breakout_split(
    entries: pd.DataFrame,
    sma_exits: pd.DataFrame,
    stop_prices: pd.DataFrame,
    close: pd.DataFrame,
    open_: pd.DataFrame,
    init_cash: float,
    vol_sma_20: pd.DataFrame,
    slippage_k: float = 0.1,
) -> list[Position]:
    """Run breakout as two half-sized portfolios per Qullamaggie's exact rules.

    Quick half (50%): sell after 5 trading days if profitable, otherwise keep
    running with original stop + SMA trail.

    Runner half (50%): trail with 10-day SMA. After day 5, if profitable, stop
    upgrades to breakeven (entry price). Original stop active before that.

    Fills at next-day open (open_.shift(-1)) to avoid same-bar lookahead.
    Slippage is size-adjusted: larger orders relative to ADV incur more impact.
    """
    half_cash = init_cash / 2

    # Compute the different exit types
    stop_exits = risk.compute_stop_exit(close, entries, stop_prices)
    partial_exits = risk.compute_partial_exit(close, entries, n_days=5)
    be_stop_exits = risk.compute_breakeven_stop_exit(
        close, entries, stop_prices, n_days=5,
    )

    # Quick half: partial profit exit OR original stop OR SMA trail
    exits_quick = partial_exits | stop_exits | sma_exits

    # Runner half: SMA trail OR breakeven stop (includes original stop for first 5 days)
    exits_runner = sma_exits | be_stop_exits

    # Half-sized positions
    sizes = risk.size_from_risk(
        close=close, stop_prices=stop_prices,
        entries=entries, equity=half_cash,
    )
    sizes = sizes.fillna(1).clip(lower=1)

    # Size-adjusted slippage: wider impact for larger orders relative to ADV
    slippage_df = risk.size_adjusted_slippage(sizes, vol_sma_20, entries, k=slippage_k)

    # Fill at next-day open to avoid same-bar lookahead bias
    fill_price = open_.shift(-1)

    positions = []
    for label, exits_df in [("breakout_quick", exits_quick), ("breakout_runner", exits_runner)]:
        _log(f"  {label}: running vectorbt...")
        pf = vbt.Portfolio.from_signals(
            close=close,
            entries=entries,
            exits=exits_df,
            price=fill_price,
            size=sizes,
            size_type="amount",
            fees=0.001,
            slippage=slippage_df,
            freq="1D",
            direction="longonly",
            init_cash=half_cash,
        )
        try:
            trades_df = pf.trades.records_readable
            pos_list = _trades_to_positions(trades_df, label)
            positions.extend(pos_list)
            _log(f"  {label}: {len(pos_list)} trades")
        except Exception as e:
            _log(f"  {label}: failed — {e}")

    _log(f"  breakout total: {len(positions)} trades (quick + runner)")
    return positions


def run_setup(
    setup_name: str,
    close: pd.DataFrame,
    open_: pd.DataFrame,
    high: pd.DataFrame,
    low: pd.DataFrame,
    volume: pd.DataFrame,
    all_indicators: dict,
    universe_mask: pd.DataFrame,
    init_cash: float,
    start: str | None,
    end: str | None,
    vol_ratio: float = 1.5,
    max_range_pct: float = 0.15,
    min_adv: float = 1_000_000,
    slippage_k: float = 0.1,
) -> list[Position]:
    """Run one setup across its universe, return list of closed Position objects."""
    _log(f"  Generating {setup_name} signals...")

    ind_data = all_indicators

    if setup_name == "breakout":
        is_pattern = pat.is_any_pattern(
            high, low, close, volume, ind_data["vol_sma_20"],
            max_range_pct=max_range_pct,
            vcp_precomputed=ind_data.get("vcp_intermediates"),
            flag_precomputed=ind_data.get("flag_intermediates"),
        )
        _consol_h = ind_data.get("consol_high")
        consol_h = _consol_h if _consol_h is not None else pat.consolidation_high(high)
        result = sig.continuation_breakout_signals(
            close=close,
            high=high,
            low=low,
            volume=volume,
            atr_14=ind_data["atr_14"],
            sma_10=ind_data["sma_10"],
            vol_sma_20=ind_data["vol_sma_20"],
            is_pattern=is_pattern,
            consol_high=consol_h,
            vol_ratio=vol_ratio,
            min_adv_dollars=min_adv,
        )
    elif setup_name == "ep":
        result = sig.episodic_pivot_signals(
            open_=open_,
            close=close,
            low=low,
            volume=volume,
            vol_sma_20=ind_data["vol_sma_20"],
            sma_10=ind_data["sma_10"],
            min_adv_dollars=min_adv,
        )
    elif setup_name == "parabolic":
        result = sig.parabolic_short_signals(
            open_=open_,
            close=close,
            high=high,
            sma_10=ind_data["sma_10"],
            sma_20=ind_data["sma_20"],
            consec_green=ind_data["consec_green"],
            pct_10d=ind_data["pct_10d"],
            vol_sma_20=ind_data["vol_sma_20"],
            min_adv_dollars=min_adv,
        )
    else:
        raise ValueError(f"Unknown setup: {setup_name}")

    entries = result["entries"].astype(bool)
    sma_exits = result["exits"].astype(bool)
    stop_prices = result["stop_prices"]

    # Apply universe filter
    entries = entries & universe_mask

    # Date range filter
    if start:
        entries = entries.loc[start:]
        sma_exits = sma_exits.loc[start:]
        stop_prices = stop_prices.loc[start:]
    if end:
        entries = entries.loc[:end]
        sma_exits = sma_exits.loc[:end]
        stop_prices = stop_prices.loc[:end]

    # Align close/open to date range
    c = close
    o = open_
    if start or end:
        slc = slice(start, end)
        c = close.loc[slc]
        o = open_.loc[slc]

    # Only pass tickers that have at least one entry (saves vectorbt overhead)
    active = entries.any(axis=0)
    active_tickers = active[active].index.tolist()
    if not active_tickers:
        _log(f"  {setup_name}: no active tickers after filtering")
        return []

    _log(f"  {setup_name}: {len(active_tickers)} active tickers")

    entries_a = entries[active_tickers]
    sma_exits_a = sma_exits[active_tickers]
    close_a = c[active_tickers]
    stop_prices_a = (
        stop_prices.loc[entries_a.index, active_tickers]
        if start or end else stop_prices[active_tickers]
    )

    # Breakout: two half-sized portfolios per Qullamaggie's partial exit rules
    if setup_name == "breakout":
        return _run_breakout_split(
            entries_a, sma_exits_a, stop_prices_a, close_a, o[active_tickers], init_cash,
            vol_sma_20=ind_data["vol_sma_20"][active_tickers],
            slippage_k=slippage_k,
        )

    # EP and parabolic: single portfolio with merged stop + SMA exits
    stop_exits_a = risk.compute_stop_exit(close_a, entries_a, stop_prices_a)
    exits_a = sma_exits_a | stop_exits_a

    sizes = risk.size_from_risk(
        close=close_a,
        stop_prices=stop_prices_a,
        entries=entries_a,
        equity=init_cash,
    )
    sizes = sizes.fillna(1).clip(lower=1)

    # Size-adjusted slippage for EP and parabolic
    slippage_df = risk.size_adjusted_slippage(
        sizes, ind_data["vol_sma_20"][active_tickers], entries_a, k=slippage_k,
    )

    direction = "shortonly" if setup_name == "parabolic" else "longonly"

    # Both EP and parabolic fill at next-day open to avoid lookahead bias
    price_df = o[active_tickers]

    pf = vbt.Portfolio.from_signals(
        close=close_a,
        entries=entries_a,
        exits=exits_a,
        price=price_df,
        size=sizes,
        size_type="amount",
        fees=0.001,          # 10bps round-trip
        slippage=slippage_df,
        freq="1D",
        direction=direction,
        init_cash=init_cash,
    )

    try:
        trades_df = pf.trades.records_readable
        positions = _trades_to_positions(trades_df, setup_name)
        _log(f"  {setup_name}: {len(positions)} trades")
        return positions
    except Exception as e:
        _log(f"  {setup_name}: failed to extract trades — {e}")
        return []


def run_backtest(args: argparse.Namespace) -> dict:
    data_dir = Path(args.data_dir)

    _log("Loading OHLCV data...")
    ohlcv = uni.load_ohlcv(data_dir)
    open_ = ohlcv["open"]
    high = ohlcv["high"]
    low = ohlcv["low"]
    close = ohlcv["close"]
    volume = ohlcv["volume"]
    _log(f"Loaded {close.shape[1]} tickers × {close.shape[0]} days")

    etf_tickers = uni.load_etf_tickers(data_dir)

    _REQUIRED_CACHE_KEYS = [
        "atr_14", "sma_10", "sma_20", "vol_sma_20",
        "ret_21", "ret_63", "ret_126",
        "dist_52w", "pct_10d", "consec_green",
        "rs_pctrank_1m", "rs_pctrank_3m", "rs_pctrank_6m",
        "vcp_num_contractions", "vcp_last_contraction_pct",
        "vcp_tightening_ratio", "vcp_vol_trend",
        "flag_pole_pct", "flag_retrace_pct", "flag_days", "flag_vol_ratio",
        "consol_high",
    ]

    if _cache.is_valid(data_dir, required_keys=_REQUIRED_CACHE_KEYS):
        _log("Loading cached indicators...")
        atr_14 = _cache.load(data_dir, "atr_14")
        sma_10 = _cache.load(data_dir, "sma_10")
        sma_20 = _cache.load(data_dir, "sma_20")
        vol_sma_20 = _cache.load(data_dir, "vol_sma_20")
        returns = {
            21: _cache.load(data_dir, "ret_21"),
            63: _cache.load(data_dir, "ret_63"),
            126: _cache.load(data_dir, "ret_126"),
        }
        dist_52w = _cache.load(data_dir, "dist_52w")
        pct_10d = _cache.load(data_dir, "pct_10d")
        consec_green = _cache.load(data_dir, "consec_green")
        # Pre-computed intermediates for fast threshold sweeps
        rs_ranks = {
            "pctrank_1m": _cache.load(data_dir, "rs_pctrank_1m"),
            "pctrank_3m": _cache.load(data_dir, "rs_pctrank_3m"),
            "pctrank_6m": _cache.load(data_dir, "rs_pctrank_6m"),
        }
        vcp_pre = {
            "num_contractions": _cache.load(data_dir, "vcp_num_contractions"),
            "last_contraction_pct": _cache.load(data_dir, "vcp_last_contraction_pct"),
            "tightening_ratio": _cache.load(data_dir, "vcp_tightening_ratio"),
            "vol_trend": _cache.load(data_dir, "vcp_vol_trend"),
        }
        flag_pre = {
            "pole_pct": _cache.load(data_dir, "flag_pole_pct"),
            "retrace_pct": _cache.load(data_dir, "flag_retrace_pct"),
            "days": _cache.load(data_dir, "flag_days"),
            "vol_ratio": _cache.load(data_dir, "flag_vol_ratio"),
        }
        consol_high_df = _cache.load(data_dir, "consol_high")
        _log("Indicators + intermediates loaded from cache")
    else:
        _log("Computing indicators (will cache for next run)...")
        atr_14 = ind.atr(high, low, close, period=14)
        sma_10 = ind.sma(close, 10)
        sma_20 = ind.sma(close, 20)
        vol_sma_20 = ind.volume_sma(volume, 20)
        returns = ind.rolling_return(close, periods=[21, 63, 126])
        dist_52w = ind.distance_from_52w_high(close, high)
        pct_10d = ind.pct_change_n_days(close, 10)
        consec_green = ind.consecutive_green_days(open_, close)

        _log("Computing pattern + RS intermediates...")
        rs_ranks = uni.rs_percentile_ranks(returns)
        vcp_pre = pat.vcp_intermediates(high, low, close, volume)
        flag_pre = pat.flag_intermediates(high, low, close, volume)
        consol_high_df = pat.consolidation_high(high)

        _log("Saving indicators + intermediates to cache...")
        _cache.save(data_dir, "atr_14", atr_14)
        _cache.save(data_dir, "sma_10", sma_10)
        _cache.save(data_dir, "sma_20", sma_20)
        _cache.save(data_dir, "vol_sma_20", vol_sma_20)
        for period, df in returns.items():
            _cache.save(data_dir, f"ret_{period}", df)
        _cache.save(data_dir, "dist_52w", dist_52w)
        _cache.save(data_dir, "pct_10d", pct_10d)
        _cache.save(data_dir, "consec_green", consec_green)
        # RS ranks
        _cache.save(data_dir, "rs_pctrank_1m", rs_ranks["pctrank_1m"])
        _cache.save(data_dir, "rs_pctrank_3m", rs_ranks["pctrank_3m"])
        _cache.save(data_dir, "rs_pctrank_6m", rs_ranks["pctrank_6m"])
        # VCP intermediates (swing-point contraction counting)
        _cache.save(data_dir, "vcp_num_contractions", vcp_pre["num_contractions"])
        _cache.save(data_dir, "vcp_last_contraction_pct", vcp_pre["last_contraction_pct"])
        _cache.save(data_dir, "vcp_tightening_ratio", vcp_pre["tightening_ratio"])
        _cache.save(data_dir, "vcp_vol_trend", vcp_pre["vol_trend"])
        # Flag intermediates (impulse + consolidation)
        _cache.save(data_dir, "flag_pole_pct", flag_pre["pole_pct"])
        _cache.save(data_dir, "flag_retrace_pct", flag_pre["retrace_pct"])
        _cache.save(data_dir, "flag_days", flag_pre["days"])
        _cache.save(data_dir, "flag_vol_ratio", flag_pre["vol_ratio"])
        # Consolidation high
        _cache.save(data_dir, "consol_high", consol_high_df)
        _cache.seal(data_dir)
        _log("Indicators ready")

    all_indicators = {
        "atr_14": atr_14,
        "sma_10": sma_10,
        "sma_20": sma_20,
        "vol_sma_20": vol_sma_20,
        "returns": returns,
        "dist_52w": dist_52w,
        "pct_10d": pct_10d,
        "consec_green": consec_green,
        # Pre-computed intermediates (used to skip rolling ops on repeated runs)
        "vcp_intermediates": vcp_pre,
        "flag_intermediates": flag_pre,
        "consol_high": consol_high_df,
    }

    _log("Building universe masks...")
    liquid = uni.mask_liquid(close, volume, vol_sma_20)
    stocks = uni.mask_stocks_only(close, etf_tickers)
    rs_top = uni.rank_relative_strength(returns, top_pct=args.rs_pct, precomputed=rs_ranks)
    near_high = uni.mask_near_52w_high(dist_52w, max_dist=args.max_dist_52w)
    prior_move = uni.mask_prior_move(all_indicators["returns"][63], min_ret=args.min_prior_move)
    ma_prox = uni.mask_ma_proximity(close, all_indicators["sma_10"], max_ext=args.max_sma_ext)

    # Market regime: only trade long setups when SPY is above its 50-day SMA
    if args.regime and "SPY" in close.columns:
        spy_sma50 = close["SPY"].rolling(50, min_periods=50).mean()
        spy_regime_ts = (close["SPY"] >= spy_sma50).reindex(close.index, fill_value=False)
        regime_mask = pd.DataFrame(
            np.tile(spy_regime_ts.values.reshape(-1, 1), (1, len(close.columns))),
            index=close.index, columns=close.columns,
        ).astype(bool)
        _log(f"  SPY regime filter: {int(spy_regime_ts.sum())} / {len(spy_regime_ts)} days bullish")
    else:
        if args.regime and "SPY" not in close.columns:
            _log("  WARNING: SPY not found — regime filter disabled")
        regime_mask = pd.DataFrame(True, index=close.index, columns=close.columns).astype(bool)

    universe_masks = {
        "breakout": uni.get_breakout_universe(
            liquid & stocks & regime_mask, rs_top, near_high,
            prior_move=prior_move, ma_proximity=ma_prox,
        ),
        "ep": uni.get_ep_universe(open_, close, volume, vol_sma_20, liquid & stocks & regime_mask),
        "parabolic": uni.get_parabolic_universe(close, open_, pct_10d, consec_green, liquid & stocks),
    }
    _log("Universe masks ready")

    setups_to_run = ["breakout", "ep", "parabolic"] if args.setup == "all" else [args.setup]

    state = PortfolioState(init_cash=args.init_cash, cash=args.init_cash, equity=args.init_cash)

    for setup_name in setups_to_run:
        _log(f"Running setup: {setup_name}")
        positions = run_setup(
            setup_name=setup_name,
            close=close,
            open_=open_,
            high=high,
            low=low,
            volume=volume,
            all_indicators=all_indicators,
            universe_mask=universe_masks[setup_name],
            init_cash=args.init_cash,
            start=args.start,
            end=args.end,
            vol_ratio=args.vol_ratio,
            max_range_pct=args.max_range,
            min_adv=args.min_adv,
            slippage_k=args.slippage_k,
        )
        state.closed_positions.extend(positions)

    _log("Generating report...")
    report = generate_report(state)
    return report


def main() -> None:
    parser = argparse.ArgumentParser(description="Qullamaggie swing trading backtest")
    parser.add_argument("--data-dir", required=True, type=str, help="Path to skill data/ directory")
    parser.add_argument("--output", default="/tmp/backtest-results.json", help="Output JSON path")
    parser.add_argument(
        "--setup", default="all",
        choices=["all", "breakout", "ep", "parabolic"],
        help="Which setup to run",
    )
    parser.add_argument("--start", default=None, help="Start date YYYY-MM-DD (optional)")
    parser.add_argument("--end", default=None, help="End date YYYY-MM-DD (optional)")
    parser.add_argument("--init-cash", type=float, default=100_000.0, help="Starting capital")
    parser.add_argument(
        "--rs-pct", type=float, default=0.02,
        help="Top RS percentile filter (0.02 = top 2%%, 0.10 = top 10%%). "
             "Qullamaggie uses 1-2%% but that yields very few signals in backtests; "
             "10%% is a practical starting point.",
    )
    parser.add_argument(
        "--vol-ratio", type=float, default=1.5,
        help="Minimum volume spike ratio on breakout day (volume / 20d avg). Default 1.5.",
    )
    parser.add_argument(
        "--max-range", type=float, default=0.15,
        help="Maximum VCP base range as fraction of high (0.15 = 15%% tight base). Default 0.15.",
    )
    parser.add_argument(
        "--max-dist-52w", type=float, default=0.25,
        help="Maximum distance below 52-week high for breakout candidates (0.25 = within 25%%). Default 0.25.",
    )
    parser.add_argument(
        "--min-adv", type=float, default=150_000_000,
        help="Min average dollar volume filter (vol_sma_20 × close). Default 150,000,000.",
    )
    parser.add_argument(
        "--slippage-k", type=float, default=0.1,
        help="Square-root market impact coefficient for size-adjusted slippage. Default 0.1.",
    )
    parser.add_argument("--min-prior-move", type=float, default=0.30,
                        help="Min 3M return for breakout entry (default: 0.30 = 30%%)")
    parser.add_argument("--max-sma-ext", type=float, default=0.10,
                        help="Max %% extension above 10-day SMA at breakout entry (default: 0.10 = 10%%)")
    parser.add_argument("--regime", default=True, action=argparse.BooleanOptionalAction,
                        help="Only trade breakout/EP when SPY > 50-day SMA (default: on)")
    args = parser.parse_args()

    t0 = time.time()
    report = run_backtest(args)
    elapsed = time.time() - t0

    save_report(report, args.output)
    _log(f"Done in {elapsed:.1f}s — results at {args.output}")

    # Print key stats
    print(json.dumps({
        "total_trades": report.get("total_trades", 0),
        "win_rate": report.get("win_rate", 0),
        "total_return": report.get("total_return", 0),
        "cagr": report.get("cagr", 0),
        "max_drawdown": report.get("max_drawdown", 0),
        "sharpe": report.get("sharpe", 0),
        "profit_factor": report.get("profit_factor", 0),
    }, indent=2))


if __name__ == "__main__":
    main()
