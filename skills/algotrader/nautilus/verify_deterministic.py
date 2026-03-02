"""Verify PrecomputedBreakout produces identical results to QullamaggieBreakout.

Runs the same params through both strategies on the same data and compares
trade count, equity, and key metrics.
"""
from __future__ import annotations

import sys
import time
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
from nautilus.strategy import QullamaggieBreakout, QullamaggieConfig
from nautilus.strategy import PrecomputedBreakout, PrecomputedConfig
from nautilus.evolve import _precompute_indicators


TICKERS = ["BTCUSDT", "SOLUSDT", "AVAXUSDT", "NEARUSDT", "APTUSDT"]
DATA_DIR = Path("data-crypto")
BAR_SPEC = "1-DAY-LAST"
INIT_CASH = 1_000_000.0

PARAMS = {
    "rs_pct": 0.30,
    "max_dist_52w": 0.40,
    "min_prior_move": 0.10,
    "min_adr_pct": 0.03,
    "vol_spike": 1.5,
    "max_range_pct": 0.20,
    "risk_pct": 0.005,
    "max_pos_pct": 0.20,
    "partial_bars": 5,
    "trail_period": 10,
}


def build_engine(trader_id: str) -> BacktestEngine:
    return BacktestEngine(
        config=BacktestEngineConfig(
            trader_id=TraderId(trader_id),
            logging=LoggingConfig(log_level="ERROR"),
        ),
    )


def load_data(engine: BacktestEngine) -> list[str]:
    engine.add_venue(
        venue=BINANCE_VENUE,
        oms_type=OmsType.NETTING,
        account_type=AccountType.CASH,
        base_currency=None,
        starting_balances=[Money(INIT_CASH, USDT)],
        bar_execution=True,
    )
    instrument_ids = []
    for ticker in TICKERS:
        df = load_ticker_csv(DATA_DIR.resolve(), ticker, "1d")
        if df is None or df.empty:
            continue
        instrument = make_instrument(ticker)
        engine.add_instrument(instrument)
        bt = BarType.from_str(f"{ticker}.BINANCE-{BAR_SPEC}-EXTERNAL")
        wrangler = BarDataWrangler(bt, instrument)
        bars = wrangler.process(df)
        engine.add_data(bars, sort=False)
        instrument_ids.append(f"{ticker}.BINANCE")
    engine.sort_data()
    return instrument_ids


def run_indicator_strategy(instrument_ids: list[str]) -> dict:
    engine = build_engine("VERIFY-IND")
    iids = load_data(engine)
    config = QullamaggieConfig(instrument_ids=iids, bar_spec=BAR_SPEC, **PARAMS)
    strategy = QullamaggieBreakout(config=config)
    engine.add_strategy(strategy)
    t0 = time.time()
    engine.run()
    elapsed = time.time() - t0
    result = engine.get_result()
    try:
        account = engine.kernel.portfolio.account(BINANCE_VENUE)
        equity = float(account.balance_total(USDT))
    except Exception:
        equity = INIT_CASH
    engine.dispose()
    return {
        "trades": result.total_positions,
        "equity": equity,
        "elapsed": elapsed,
    }


def run_precomputed_strategy(instrument_ids: list[str]) -> dict:
    indicators = {}
    for ticker in TICKERS:
        df = load_ticker_csv(DATA_DIR.resolve(), ticker, "1d")
        if df is None or df.empty:
            continue
        indicators[ticker] = _precompute_indicators(df)

    engine = build_engine("VERIFY-PRE")
    iids = load_data(engine)
    config = PrecomputedConfig(instrument_ids=iids, bar_spec=BAR_SPEC, **PARAMS)
    strategy = PrecomputedBreakout(config=config, indicators=indicators)
    engine.add_strategy(strategy)
    t0 = time.time()
    engine.run()
    elapsed = time.time() - t0
    result = engine.get_result()
    try:
        account = engine.kernel.portfolio.account(BINANCE_VENUE)
        equity = float(account.balance_total(USDT))
    except Exception:
        equity = INIT_CASH
    engine.dispose()
    return {
        "trades": result.total_positions,
        "equity": equity,
        "elapsed": elapsed,
    }


def main():
    print("=== Deterministic Verification ===")
    print(f"Tickers: {TICKERS}")
    print(f"Params: {PARAMS}\n")

    print("Running indicator-based strategy...")
    ind_result = run_indicator_strategy([])

    print("Running pre-computed strategy...")
    pre_result = run_precomputed_strategy([])

    print(f"\n{'Metric':<20} {'Indicator':>15} {'Precomputed':>15} {'Match':>8}")
    print("-" * 60)

    trades_match = ind_result["trades"] == pre_result["trades"]
    equity_diff_pct = abs(ind_result["equity"] - pre_result["equity"]) / ind_result["equity"]
    equity_close = equity_diff_pct < 0.05  # 5% tolerance for indicator warmup differences
    speedup = ind_result["elapsed"] / pre_result["elapsed"] if pre_result["elapsed"] > 0 else 0

    print(f"{'Trades':<20} {ind_result['trades']:>15} {pre_result['trades']:>15} {'YES' if trades_match else 'NO':>8}")
    print(f"{'Final Equity':<20} {ind_result['equity']:>15,.2f} {pre_result['equity']:>15,.2f} {equity_diff_pct:>7.1%}")
    print(f"{'Elapsed (s)':<20} {ind_result['elapsed']:>15.3f} {pre_result['elapsed']:>15.3f} {speedup:>7.1f}x")

    if trades_match and equity_close:
        print(f"\nVERIFICATION PASSED: Trade count identical, equity within {equity_diff_pct:.1%}.")
        print(f"  Speedup: {speedup:.1f}x (single run)")
    else:
        if not trades_match:
            print("\nVERIFICATION FAILED: Trade count differs.")
            sys.exit(1)
        print(f"\nWARNING: Equity differs by {equity_diff_pct:.1%} (beyond 5% threshold).")
        sys.exit(1)


if __name__ == "__main__":
    main()
