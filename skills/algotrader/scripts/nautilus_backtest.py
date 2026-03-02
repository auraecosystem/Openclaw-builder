#!/usr/bin/env python3
"""Load our Binance kline CSVs into NautilusTrader's BacktestEngine.

Usage:
    python nautilus_backtest.py --data-dir ../data-crypto --tickers BTCUSDT,ETHUSDT
    python nautilus_backtest.py --data-dir ../data-crypto --timeframe 5m --tickers BTCUSDT
    python nautilus_backtest.py --data-dir ../data-crypto  # all 100 tickers, daily
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from decimal import Decimal
from pathlib import Path

import pandas as pd

from nautilus_trader.adapters.binance import BINANCE_VENUE
from nautilus_trader.backtest.engine import BacktestEngine
from nautilus_trader.common.enums import LogColor
from nautilus_trader.config import BacktestEngineConfig
from nautilus_trader.config import LoggingConfig
from nautilus_trader.model.currencies import USDT
from nautilus_trader.model.currencies import register_currency
from nautilus_trader.model.data import Bar
from nautilus_trader.model.data import BarType
from nautilus_trader.model.enums import AccountType
from nautilus_trader.model.enums import CurrencyType
from nautilus_trader.model.enums import OmsType
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import Symbol
from nautilus_trader.model.identifiers import TraderId
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.instruments.currency_pair import CurrencyPair
from nautilus_trader.model.objects import Currency
from nautilus_trader.model.objects import Money
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity
from nautilus_trader.persistence.wranglers import BarDataWrangler
from nautilus_trader.trading.strategy import Strategy


# --- Currency registration ---

def ensure_currency(code: str) -> Currency:
    """Get currency from internal map, or register it as a new crypto currency."""
    existing = Currency.from_internal_map(code)
    if existing is not None:
        return existing
    c = Currency(
        code=code,
        precision=8,
        iso4217=0,
        name=code,
        currency_type=CurrencyType.CRYPTO,
    )
    register_currency(c)
    return c


# --- Instrument factory ---

def make_instrument(symbol: str) -> CurrencyPair:
    """Create a CurrencyPair instrument for a Binance USDT spot pair."""
    base_code = symbol.removesuffix("USDT")
    base = ensure_currency(base_code)
    # size_precision=0: some meme coins have volumes > 95 trillion which overflows
    # NautilusTrader's QUANTITY_MAX (34T) at higher precisions. We don't trade,
    # so volume precision doesn't matter — just needs to fit.
    return CurrencyPair(
        instrument_id=InstrumentId(Symbol(symbol), Venue("BINANCE")),
        raw_symbol=Symbol(symbol),
        base_currency=base,
        quote_currency=USDT,
        price_precision=8,
        size_precision=0,
        price_increment=Price(1e-8, precision=8),
        size_increment=Quantity(1, precision=0),
        lot_size=None,
        max_quantity=None,
        min_quantity=None,
        max_notional=None,
        min_notional=None,
        max_price=None,
        min_price=None,
        margin_init=Decimal(0),
        margin_maint=Decimal(0),
        maker_fee=Decimal("0.001"),
        taker_fee=Decimal("0.001"),
        ts_event=0,
        ts_init=0,
    )


# --- CSV loading ---

BINANCE_KLINE_COLS = [
    "open_time", "open", "high", "low", "close", "volume",
    "close_time", "quote_volume", "trades", "taker_buy_vol",
    "taker_buy_quote_vol", "ignore",
]

TF_GLOB = {"1d": "-1d-", "5m": "-5m-"}


def load_ticker_csv(data_dir: Path, ticker: str, timeframe: str) -> pd.DataFrame | None:
    """Load all monthly CSVs for one ticker, return OHLCV DataFrame with timestamp index."""
    raw_dir = data_dir / "raw" / ticker
    if not raw_dir.exists():
        return None

    pattern = TF_GLOB.get(timeframe, f"-{timeframe}-")
    csvs = sorted(f for f in raw_dir.iterdir() if pattern in f.name and f.suffix == ".csv")
    if not csvs:
        return None

    frames = []
    for csv_path in csvs:
        df = pd.read_csv(csv_path, header=None, names=BINANCE_KLINE_COLS)
        frames.append(df)

    combined = pd.concat(frames, ignore_index=True)
    combined.sort_values("open_time", inplace=True)
    combined.drop_duplicates(subset="open_time", inplace=True)

    # Normalize timestamps: older Binance files use ms (13 digits), newer use us (16 digits)
    ts = combined["open_time"].values.copy()
    mask = ts > 9_999_999_999_999
    ts[mask] = ts[mask] // 1000
    combined["timestamp"] = pd.to_datetime(ts, unit="ms", utc=True)

    combined = combined.set_index("timestamp")[["open", "high", "low", "close", "volume"]]
    for col in ["open", "high", "low", "close", "volume"]:
        combined[col] = pd.to_numeric(combined[col], errors="coerce")

    # Clamp volume to NautilusTrader's QUANTITY_MAX (meme coins like SHIB exceed it)
    QUANTITY_MAX = 34_028_236_692_093.0
    combined["volume"] = combined["volume"].clip(upper=QUANTITY_MAX)

    return combined


# --- Trivial verification strategy ---

class BarCounterStrategy(Strategy):
    """Counts bars per instrument. Used to verify data loaded correctly."""

    def __init__(self):
        super().__init__()
        self.counts: dict[str, int] = {}

    def on_start(self):
        self.log.info("BarCounterStrategy started")

    def on_bar(self, bar: Bar):
        key = str(bar.bar_type.instrument_id)
        self.counts[key] = self.counts.get(key, 0) + 1

    def on_stop(self):
        total = sum(self.counts.values())
        self.log.info(f"Total bars processed: {total}", color=LogColor.GREEN)
        for sym, count in sorted(self.counts.items()):
            self.log.info(f"  {sym}: {count} bars")


# --- Main ---

def main():
    parser = argparse.ArgumentParser(description="Run NautilusTrader backtest on Binance data")
    parser.add_argument("--data-dir", type=Path, default=Path("../data-crypto"))
    parser.add_argument("--timeframe", default="1d", choices=["1d", "5m"])
    parser.add_argument("--tickers", help="Comma-separated subset of tickers")
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
        unknown = set(symbols) - set(all_symbols)
        if unknown:
            print(f"Warning: unknown tickers: {unknown}", file=sys.stderr)
    else:
        symbols = all_symbols

    # Bar spec string for NautilusTrader
    bar_spec = "1-DAY-LAST" if args.timeframe == "1d" else "5-MINUTE-LAST"

    # Configure engine
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

    # Load data
    bar_types_for_strategy = []
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

        bar_types_for_strategy.append(bt)
        print(f"  [{i+1}/{len(symbols)}] {symbol}: {len(bars)} bars loaded")

    if not bar_types_for_strategy:
        print("Error: no data loaded", file=sys.stderr)
        sys.exit(1)

    print(f"\nData loaded in {time.time() - t0:.1f}s")
    print(f"Sorting {len(bar_types_for_strategy)} instruments...")
    engine.sort_data()

    # Add strategy and subscribe to all bar types
    strategy = BarCounterStrategy()
    engine.add_strategy(strategy)
    for bt in bar_types_for_strategy:
        strategy.subscribe_bars(bt)

    # Run
    print(f"\nRunning backtest...")
    t1 = time.time()
    engine.run()
    elapsed = time.time() - t1
    print(f"\nBacktest completed in {elapsed:.1f}s")

    # Summary
    total_bars = sum(strategy.counts.values())
    print(f"Total bars processed: {total_bars}")
    print(f"Throughput: {total_bars / elapsed:,.0f} bars/sec")

    engine.dispose()


if __name__ == "__main__":
    main()
