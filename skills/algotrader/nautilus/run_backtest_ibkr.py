"""Run Qullamaggie breakout strategy backtest on IBKR US equity data.

Loads parquet files from data-ibkr/ (fetched via scripts/ibkr/fetch_bars.py)
into NautilusTrader's BacktestEngine.

Usage:
    python -m nautilus.run_backtest_ibkr
    python -m nautilus.run_backtest_ibkr --profile ibkr_rotation
    python -m nautilus.run_backtest_ibkr --benchmark SPY --cash 100000
"""

from __future__ import annotations

import argparse
import sys
import time
from decimal import Decimal
from pathlib import Path

import pandas as pd

from nautilus_trader.backtest.engine import BacktestEngine
from nautilus_trader.config import BacktestEngineConfig, LoggingConfig
from nautilus_trader.model.currencies import USD
from nautilus_trader.model.data import BarType
from nautilus_trader.model.enums import AccountType, OmsType
from nautilus_trader.model.identifiers import InstrumentId, Symbol, Venue
from nautilus_trader.model.identifiers import TraderId
from nautilus_trader.model.instruments import Equity
from nautilus_trader.model.objects import Money, Price, Quantity
from nautilus_trader.persistence.wranglers import BarDataWrangler

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from trading_config import load_trading_config

from nautilus.strategy import QullamaggieBreakout, QullamaggieConfig

VENUE = Venue("IBKR")


def make_equity(symbol: str) -> Equity:
    """Create an Equity instrument for a US stock on the IBKR venue."""
    return Equity(
        instrument_id=InstrumentId(Symbol(symbol), VENUE),
        raw_symbol=Symbol(symbol),
        currency=USD,
        price_precision=2,
        price_increment=Price.from_str("0.01"),
        lot_size=Quantity.from_int(1),
        ts_event=0,
        ts_init=0,
    )


def load_ibkr_parquet(path: Path, tickers: list[str] | None = None) -> dict[str, pd.DataFrame]:
    """Load IBKR parquet file and return {symbol: ohlcv_df} with DatetimeIndex."""
    df = pd.read_parquet(path)

    if tickers:
        tickers_upper = [t.upper() for t in tickers]
        df = df[df["symbol"].isin(tickers_upper)]

    result = {}
    for symbol, group in df.groupby("symbol"):
        ohlcv = group.copy()
        ohlcv["timestamp"] = pd.to_datetime(ohlcv["date"], utc=True)
        ohlcv = ohlcv.set_index("timestamp")[["open", "high", "low", "close", "volume"]]
        ohlcv = ohlcv.sort_index().drop_duplicates()
        for col in ["open", "high", "low", "close", "volume"]:
            ohlcv[col] = pd.to_numeric(ohlcv[col], errors="coerce")
        result[str(symbol)] = ohlcv

    return result


def main():
    parser = argparse.ArgumentParser(
        description="Qullamaggie backtest on IBKR US equity data",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
examples:
  python -m nautilus.run_backtest_ibkr
  python -m nautilus.run_backtest_ibkr --profile ibkr_rotation
  python -m nautilus.run_backtest_ibkr --tickers SPY,AAPL,NVDA --cash 500000
  python -m nautilus.run_backtest_ibkr --benchmark SPY
""",
    )
    parser.add_argument("--profile", default="ibkr_rotation", help="Configured dataset profile")
    parser.add_argument("--tickers", help="Comma-separated tickers (default: all in file)")
    parser.add_argument("--benchmark", default="SPY", help="Benchmark symbol for regime filter")
    parser.add_argument("--cash", type=float, default=1_000_000, help="Starting cash in USD")
    parser.add_argument("--bar-spec", default="1-DAY-LAST")
    parser.add_argument("--log-level", default="INFO")
    args = parser.parse_args()

    cfg = load_trading_config()
    profile = cfg.profile(args.profile)
    data_path = profile.ohlcv
    if data_path is None or not data_path.exists():
        print(f"Error: configured IBKR parquet not found: {data_path}", file=sys.stderr)
        sys.exit(1)

    tickers = [t.strip().upper() for t in args.tickers.split(",")] if args.tickers else None

    # Load data
    t0 = time.time()
    symbol_data = load_ibkr_parquet(data_path, tickers)
    if not symbol_data:
        print("Error: no data loaded", file=sys.stderr)
        sys.exit(1)

    # Configure engine
    engine = BacktestEngine(
        config=BacktestEngineConfig(
            trader_id=TraderId("BACKTEST-001"),
            logging=LoggingConfig(log_level=args.log_level),
        ),
    )

    engine.add_venue(
        venue=VENUE,
        oms_type=OmsType.NETTING,
        account_type=AccountType.MARGIN,
        base_currency=USD,
        starting_balances=[Money(args.cash, USD)],
        bar_execution=True,
    )

    # Load instruments and bars
    instrument_ids = []
    for symbol, ohlcv in symbol_data.items():
        instrument = make_equity(symbol)
        engine.add_instrument(instrument)

        bt = BarType.from_str(f"{symbol}.IBKR-{args.bar_spec}-EXTERNAL")
        wrangler = BarDataWrangler(bt, instrument)
        bars = wrangler.process(ohlcv)
        engine.add_data(bars, sort=False)

        instrument_ids.append(f"{symbol}.IBKR")
        print(f"  {symbol}: {len(bars)} bars ({ohlcv.index[0].date()} → {ohlcv.index[-1].date()})")

    print(f"\nData loaded in {time.time() - t0:.1f}s ({len(instrument_ids)} instruments)")
    engine.sort_data()

    # Benchmark must be in the instrument list for regime filter
    benchmark_id = f"{args.benchmark}.IBKR"
    if benchmark_id not in instrument_ids:
        print(f"Warning: benchmark {args.benchmark} not in data — regime filter disabled",
              file=sys.stderr)

    config = QullamaggieConfig(
        instrument_ids=instrument_ids,
        bar_spec=args.bar_spec,
        venue="IBKR",
        benchmark_id=benchmark_id,
        quote_currency="USD",
    )
    strategy = QullamaggieBreakout(config=config)
    engine.add_strategy(strategy)

    print(f"\nRunning backtest with {len(instrument_ids)} instruments...")
    t1 = time.time()
    engine.run()
    elapsed = time.time() - t1
    print(f"\nBacktest completed in {elapsed:.1f}s")

    print("\n--- Account Report ---")
    print(engine.trader.generate_account_report(VENUE))
    print("\n--- Order Fills ---")
    print(engine.trader.generate_order_fills_report())
    print("\n--- Positions ---")
    print(engine.trader.generate_positions_report())

    engine.dispose()


if __name__ == "__main__":
    main()
