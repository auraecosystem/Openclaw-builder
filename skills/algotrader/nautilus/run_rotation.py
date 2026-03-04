"""Run cross-sectional rotation momentum backtest via NautilusTrader.

Supports both crypto (Binance CSVs) and US equities (IBKR parquet).

Usage:
    # Crypto (Binance)
    python -m nautilus.run_rotation --data-dir data-crypto/ --venue BINANCE --currency USDT

    # US equities (IBKR)
    python -m nautilus.run_rotation --data-dir data/ --ibkr-data data-ibkr/daily.parquet

    # With custom rotation params
    python -m nautilus.run_rotation --data-dir data-crypto/ --top-n 10 --rebalance 5
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from decimal import Decimal
from pathlib import Path

import pandas as pd

from nautilus_trader.backtest.engine import BacktestEngine
from nautilus_trader.config import BacktestEngineConfig, LoggingConfig
from nautilus_trader.model.currencies import USD, USDT
from nautilus_trader.model.data import BarType
from nautilus_trader.model.enums import AccountType, OmsType
from nautilus_trader.model.identifiers import InstrumentId, Symbol, TraderId, Venue
from nautilus_trader.model.instruments import Equity
from nautilus_trader.model.instruments.currency_pair import CurrencyPair
from nautilus_trader.model.objects import Currency, Money, Price, Quantity
from nautilus_trader.persistence.wranglers import BarDataWrangler

from nautilus.rotation_strategy import RotationConfig, RotationStrategy

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))


def make_equity(symbol: str, venue: str) -> Equity:
    """Create an Equity instrument."""
    return Equity(
        instrument_id=InstrumentId(Symbol(symbol), Venue(venue)),
        raw_symbol=Symbol(symbol),
        currency=USD,
        price_precision=2,
        price_increment=Price.from_str("0.01"),
        lot_size=Quantity.from_int(1),
        ts_event=0,
        ts_init=0,
    )


def make_crypto(symbol: str) -> CurrencyPair:
    """Create a CurrencyPair instrument for Binance."""
    from nautilus_trader.model.enums import CurrencyType
    from nautilus_trader.model.currencies import register_currency

    base_code = symbol.removesuffix("USDT")
    existing = Currency.from_internal_map(base_code)
    if existing is None:
        c = Currency(code=base_code, precision=8, iso4217=0,
                     name=base_code, currency_type=CurrencyType.CRYPTO)
        register_currency(c)
        existing = c

    return CurrencyPair(
        instrument_id=InstrumentId(Symbol(symbol), Venue("BINANCE")),
        raw_symbol=Symbol(symbol),
        base_currency=existing,
        quote_currency=USDT,
        price_precision=8,
        size_precision=0,
        price_increment=Price(1e-8, precision=8),
        size_increment=Quantity(1, precision=0),
        lot_size=None, max_quantity=None, min_quantity=None,
        max_notional=None, min_notional=None,
        max_price=None, min_price=None,
        margin_init=Decimal(0), margin_maint=Decimal(0),
        maker_fee=Decimal("0.001"), taker_fee=Decimal("0.001"),
        ts_event=0, ts_init=0,
    )


def load_crypto_data(data_dir: Path, tickers: list[str], bar_spec: str) -> list[tuple]:
    """Load Binance CSV data. Returns [(instrument, bar_type, bars), ...]."""
    BINANCE_KLINE_COLS = [
        "open_time", "open", "high", "low", "close", "volume",
        "close_time", "quote_volume", "trades", "taker_buy_vol",
        "taker_buy_quote_vol", "ignore",
    ]

    results = []
    for ticker in tickers:
        raw_dir = data_dir / "raw" / ticker
        if not raw_dir.exists():
            continue

        csvs = sorted(f for f in raw_dir.iterdir() if "-1d-" in f.name and f.suffix == ".csv")
        if not csvs:
            continue

        frames = [pd.read_csv(f, header=None, names=BINANCE_KLINE_COLS) for f in csvs]
        combined = pd.concat(frames, ignore_index=True)
        combined.sort_values("open_time", inplace=True)
        combined.drop_duplicates(subset="open_time", inplace=True)

        ts = combined["open_time"].values.copy()
        mask = ts > 9_999_999_999_999
        ts[mask] = ts[mask] // 1000
        combined["timestamp"] = pd.to_datetime(ts, unit="ms", utc=True)
        ohlcv = combined.set_index("timestamp")[["open", "high", "low", "close", "volume"]]
        for col in ohlcv.columns:
            ohlcv[col] = pd.to_numeric(ohlcv[col], errors="coerce")

        instrument = make_crypto(ticker)
        bt = BarType.from_str(f"{ticker}.BINANCE-{bar_spec}-EXTERNAL")
        wrangler = BarDataWrangler(bt, instrument)
        bars = wrangler.process(ohlcv)

        results.append((instrument, bt, bars))

    return results


def load_ibkr_data(parquet_path: Path, venue: str, bar_spec: str,
                   tickers: list[str] | None = None) -> list[tuple]:
    """Load IBKR parquet data. Returns [(instrument, bar_type, bars), ...]."""
    df = pd.read_parquet(parquet_path)
    if tickers:
        df = df[df["symbol"].isin([t.upper() for t in tickers])]

    results = []
    for symbol, group in df.groupby("symbol"):
        ohlcv = group.copy()
        ohlcv["timestamp"] = pd.to_datetime(ohlcv["date"], utc=True)
        ohlcv = ohlcv.set_index("timestamp")[["open", "high", "low", "close", "volume"]]
        ohlcv = ohlcv.sort_index().drop_duplicates()
        for col in ohlcv.columns:
            ohlcv[col] = pd.to_numeric(ohlcv[col], errors="coerce")

        instrument = make_equity(str(symbol), venue)
        bt = BarType.from_str(f"{symbol}.{venue}-{bar_spec}-EXTERNAL")
        wrangler = BarDataWrangler(bt, instrument)
        bars = wrangler.process(ohlcv)

        results.append((instrument, bt, bars))

    return results


def load_wide_parquet_data(data_dir: Path, venue: str, bar_spec: str) -> list[tuple]:
    """Load wide ohlcv.parquet and convert each ticker to NT bars."""
    from lib.universe import load_ohlcv

    ohlcv = load_ohlcv(data_dir)
    close_df = ohlcv["close"]
    tickers = list(close_df.columns)

    results = []
    for i, ticker in enumerate(tickers):
        # Extract single-ticker OHLCV
        single = pd.DataFrame({
            "open": ohlcv["open"][ticker],
            "high": ohlcv["high"][ticker],
            "low": ohlcv["low"][ticker],
            "close": ohlcv["close"][ticker],
            "volume": ohlcv["volume"][ticker],
        })

        # Drop rows where close is NaN (ticker not listed yet)
        single = single.dropna(subset=["close"])
        if len(single) < 50:
            continue

        # Ensure UTC DatetimeIndex
        if single.index.tz is None:
            single.index = single.index.tz_localize("UTC")

        instrument = make_equity(ticker, venue)
        bt = BarType.from_str(f"{ticker}.{venue}-{bar_spec}-EXTERNAL")
        wrangler = BarDataWrangler(bt, instrument)
        bars = wrangler.process(single)

        results.append((instrument, bt, bars))

        if (i + 1) % 500 == 0:
            print(f"  Loaded {i + 1}/{len(tickers)} tickers...")

    return results


def main():
    parser = argparse.ArgumentParser(description="Rotation momentum backtest")
    parser.add_argument("--data-dir", type=Path, required=True,
                        help="Data directory with ohlcv.parquet and cache/")
    parser.add_argument("--ibkr-data", type=Path, default=None,
                        help="IBKR parquet file (overrides wide parquet loading)")
    parser.add_argument("--venue", default="XNYS", help="Venue name (XNYS, BINANCE, IBKR)")
    parser.add_argument("--currency", default="USD", help="Quote currency (USD, USDT)")
    parser.add_argument("--bar-spec", default="1-DAY-LAST")
    parser.add_argument("--cash", type=float, default=1_000_000)
    parser.add_argument("--tickers", help="Comma-separated tickers (default: all)")
    parser.add_argument("--start", help="Start date YYYY-MM-DD")
    parser.add_argument("--end", help="End date YYYY-MM-DD")

    # Rotation params
    parser.add_argument("--top-n", type=int, default=20)
    parser.add_argument("--rebalance", type=int, default=5, help="Rebalance every N bars")
    parser.add_argument("--risk-pct", type=float, default=0.01)
    parser.add_argument("--max-pos-pct", type=float, default=0.10)
    parser.add_argument("--w-rs", type=float, default=0.4)
    parser.add_argument("--w-pattern", type=float, default=0.3)
    parser.add_argument("--w-signal", type=float, default=0.3)

    parser.add_argument("--log-level", default="WARNING")
    args = parser.parse_args()

    data_dir = args.data_dir.resolve()
    cache_dir = data_dir / "cache"

    # Determine currency
    quote_currency = Currency.from_str(args.currency)

    # Load data
    t0 = time.time()
    print(f"Loading data from {data_dir}...")

    tickers_filter = [t.strip().upper() for t in args.tickers.split(",")] if args.tickers else None

    if args.ibkr_data:
        data = load_ibkr_data(args.ibkr_data, args.venue, args.bar_spec, tickers_filter)
    elif args.venue == "BINANCE":
        universe_path = data_dir / "universe.json"
        if universe_path.exists():
            with open(universe_path) as f:
                universe = json.load(f)
            all_symbols = [p["symbol"] for p in universe["pairs"]]
        else:
            all_symbols = tickers_filter or []
        symbols = tickers_filter or all_symbols
        data = load_crypto_data(data_dir, symbols, args.bar_spec)
    else:
        data = load_wide_parquet_data(data_dir, args.venue, args.bar_spec)

    if not data:
        print("Error: no data loaded", file=sys.stderr)
        sys.exit(1)

    print(f"Loaded {len(data)} instruments in {time.time() - t0:.1f}s")

    # Build engine
    engine = BacktestEngine(
        config=BacktestEngineConfig(
            trader_id=TraderId("ROTATION-001"),
            logging=LoggingConfig(log_level=args.log_level),
        ),
    )

    engine.add_venue(
        venue=Venue(args.venue),
        oms_type=OmsType.NETTING,
        account_type=AccountType.MARGIN,
        base_currency=quote_currency,
        starting_balances=[Money(args.cash, quote_currency)],
        bar_execution=True,
    )

    instrument_ids = []
    for instrument, bt, bars in data:
        engine.add_instrument(instrument)
        engine.add_data(bars, sort=False)
        instrument_ids.append(str(instrument.id))

    engine.sort_data()

    # Configure rotation strategy
    config = RotationConfig(
        instrument_ids=instrument_ids,
        bar_spec=args.bar_spec,
        cache_dir=str(cache_dir),
        venue=args.venue,
        quote_currency=args.currency,
        top_n=args.top_n,
        rebalance_bars=args.rebalance,
        risk_pct=args.risk_pct,
        max_pos_pct=args.max_pos_pct,
        w_rs=args.w_rs,
        w_pattern=args.w_pattern,
        w_signal=args.w_signal,
    )
    strategy = RotationStrategy(config=config)
    engine.add_strategy(strategy)

    # Run
    print(f"\nRunning rotation backtest with {len(instrument_ids)} instruments, "
          f"top_n={args.top_n}, rebalance every {args.rebalance} bars...")
    t1 = time.time()
    engine.run()
    elapsed = time.time() - t1
    print(f"\nBacktest completed in {elapsed:.1f}s")

    # Reports
    venue = Venue(args.venue)
    print("\n--- Account Report ---")
    print(engine.trader.generate_account_report(venue))
    print("\n--- Order Fills ---")
    fills = engine.trader.generate_order_fills_report()
    print(fills)
    print(f"\nTotal fills: {len(fills)}")
    print("\n--- Positions ---")
    positions = engine.trader.generate_positions_report()
    print(positions)
    print(f"\nTotal positions: {len(positions)}")

    engine.dispose()


if __name__ == "__main__":
    main()
