"""Run Qullamaggie breakout strategy backtest via NautilusTrader.

Usage:
    python -m nautilus.run_backtest --data-dir ../data-crypto
    python -m nautilus.run_backtest --data-dir ../data-crypto --tickers BTCUSDT,SOLUSDT
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from decimal import Decimal
from pathlib import Path

from nautilus_trader.adapters.binance import BINANCE_VENUE
from nautilus_trader.backtest.engine import BacktestEngine
from nautilus_trader.config import BacktestEngineConfig, LoggingConfig
from nautilus_trader.model.currencies import USDT
from nautilus_trader.model.data import BarType
from nautilus_trader.model.enums import AccountType, OmsType
from nautilus_trader.model.identifiers import TraderId
from nautilus_trader.model.objects import Money
from nautilus_trader.persistence.wranglers import BarDataWrangler

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from scripts.nautilus_backtest import ensure_currency, load_ticker_csv, make_instrument

from nautilus.strategy import QullamaggieBreakout, QullamaggieConfig

DEFAULT_TICKERS = [
    "SOLUSDT", "AVAXUSDT", "NEARUSDT", "APTUSDT", "INJUSDT",
    "SUIUSDT", "FETUSDT", "RNDRUSDT", "SEIUSDT", "TIAUSDT",
]


def main():
    parser = argparse.ArgumentParser(description="Qullamaggie NautilusTrader backtest")
    parser.add_argument("--data-dir", type=Path, default=Path("../data-crypto"))
    parser.add_argument("--timeframe", default="1d", choices=["1d", "5m"])
    parser.add_argument("--tickers", help="Comma-separated tickers (always includes BTCUSDT)")
    parser.add_argument("--log-level", default="INFO")
    args = parser.parse_args()

    data_dir = args.data_dir.resolve()
    universe_path = data_dir / "universe.json"
    if not universe_path.exists():
        print(f"Error: {universe_path} not found", file=sys.stderr)
        sys.exit(1)

    with open(universe_path) as f:
        universe = json.load(f)
    all_symbols = [p["symbol"] for p in universe["pairs"]]

    if args.tickers:
        symbols = [s.strip().upper() for s in args.tickers.split(",")]
    else:
        symbols = [s for s in DEFAULT_TICKERS if s in all_symbols]

    if "BTCUSDT" not in symbols:
        symbols.insert(0, "BTCUSDT")

    bar_spec = "1-DAY-LAST" if args.timeframe == "1d" else "5-MINUTE-LAST"

    engine = BacktestEngine(
        config=BacktestEngineConfig(
            trader_id=TraderId("BACKTEST-001"),
            logging=LoggingConfig(log_level=args.log_level),
        ),
    )

    engine.add_venue(
        venue=BINANCE_VENUE,
        oms_type=OmsType.NETTING,
        account_type=AccountType.CASH,
        base_currency=None,
        starting_balances=[Money(1_000_000, USDT)],
        bar_execution=True,
    )

    instrument_ids = []
    t0 = time.time()

    for i, symbol in enumerate(symbols):
        df = load_ticker_csv(data_dir, symbol, args.timeframe)
        if df is None or df.empty:
            print(f"  [{i+1}/{len(symbols)}] {symbol}: no data, skipping")
            continue

        instrument = make_instrument(symbol)
        engine.add_instrument(instrument)

        bt = BarType.from_str(f"{symbol}.BINANCE-{bar_spec}-EXTERNAL")
        wrangler = BarDataWrangler(bt, instrument)
        bars = wrangler.process(df)
        engine.add_data(bars, sort=False)

        instrument_ids.append(f"{symbol}.BINANCE")
        print(f"  [{i+1}/{len(symbols)}] {symbol}: {len(bars)} bars loaded")

    if not instrument_ids:
        print("Error: no data loaded", file=sys.stderr)
        sys.exit(1)

    print(f"\nData loaded in {time.time() - t0:.1f}s")
    engine.sort_data()

    config = QullamaggieConfig(
        instrument_ids=instrument_ids,
        bar_spec=bar_spec,
    )
    strategy = QullamaggieBreakout(config=config)
    engine.add_strategy(strategy)

    print(f"\nRunning backtest with {len(instrument_ids)} instruments...")
    t1 = time.time()
    engine.run()
    elapsed = time.time() - t1
    print(f"\nBacktest completed in {elapsed:.1f}s")

    print("\n--- Account Report ---")
    print(engine.trader.generate_account_report(BINANCE_VENUE))
    print("\n--- Order Fills ---")
    print(engine.trader.generate_order_fills_report())
    print("\n--- Positions ---")
    print(engine.trader.generate_positions_report())

    engine.dispose()


if __name__ == "__main__":
    main()
