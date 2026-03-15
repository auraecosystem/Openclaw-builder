#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["pandas", "pyarrow", "numpy", "nautilus_trader"]
# ///
"""Convert wide ohlcv.parquet (MultiIndex columns) to NautilusTrader ParquetDataCatalog.

One-time conversion job. Iterates tickers sequentially — I/O bound, not CPU bound.

Usage:
    uv run scripts/build_nt_catalog.py
    uv run scripts/build_nt_catalog.py --profile equities_daily
"""

from __future__ import annotations

import argparse
import sys
import time
from decimal import Decimal
from pathlib import Path

import pandas as pd

# Allow `lib.universe` imports from the project root
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from lib.universe import load_ohlcv
from trading_config import load_trading_config

from nautilus_trader.model.currencies import USD
from nautilus_trader.model.data import BarType
from nautilus_trader.model.identifiers import InstrumentId, Symbol, Venue
from nautilus_trader.model.instruments import Equity
from nautilus_trader.model.objects import Price, Quantity
from nautilus_trader.persistence.catalog import ParquetDataCatalog
from nautilus_trader.persistence.wranglers import BarDataWrangler


def make_equity(symbol: str, venue: Venue) -> Equity:
    """Create a minimal US equity instrument for catalog ingestion."""
    return Equity(
        instrument_id=InstrumentId(Symbol(symbol), venue),
        raw_symbol=Symbol(symbol),
        currency=USD,
        price_precision=2,
        price_increment=Price.from_str("0.01"),
        lot_size=Quantity.from_int(1),
        ts_event=0,
        ts_init=0,
    )


def extract_ticker_ohlcv(ohlcv: dict[str, pd.DataFrame], ticker: str) -> pd.DataFrame:
    """Pull a single-ticker OHLCV DataFrame from the wide field dict.

    The wide dict has shape {field: DatetimeIndex x tickers}. We slice one
    column per field and reassemble with the standard column order expected
    by BarDataWrangler.process().
    """
    df = pd.DataFrame({
        "open":   ohlcv["open"][ticker],
        "high":   ohlcv["high"][ticker],
        "low":    ohlcv["low"][ticker],
        "close":  ohlcv["close"][ticker],
        "volume": ohlcv["volume"][ticker],
    })

    # Drop rows where every OHLCV value is NaN (delisted / not yet listed periods)
    df = df.dropna(how="all")
    # Also require a valid close price — the candle is unusable without it
    df = df.dropna(subset=["close"])

    # Ensure UTC-aware DatetimeIndex; BarDataWrangler requires UTC timestamps
    if df.index.tz is None:
        df.index = df.index.tz_localize("UTC")
    else:
        df.index = df.index.tz_convert("UTC")

    return df.sort_index()


def build_catalog(
    data_dir: Path,
    output_dir: Path,
    venue_str: str,
    bar_spec: str,
) -> None:
    venue = Venue(venue_str)
    catalog = ParquetDataCatalog(str(output_dir))

    print(f"Loading ohlcv.parquet from {data_dir} ...")
    t0 = time.time()
    ohlcv = load_ohlcv(data_dir)
    tickers = list(ohlcv["close"].columns)
    print(f"  {len(tickers)} tickers loaded in {time.time() - t0:.1f}s")

    skipped = 0
    written = 0
    t_start = time.time()

    for i, ticker in enumerate(tickers, start=1):
        ticker_df = extract_ticker_ohlcv(ohlcv, ticker)

        if ticker_df.empty:
            skipped += 1
            continue

        instrument = make_equity(ticker, venue)
        bar_type = BarType.from_str(f"{ticker}.{venue_str}-{bar_spec}-EXTERNAL")
        wrangler = BarDataWrangler(bar_type, instrument)
        bars = wrangler.process(ticker_df)

        # write_data handles both Equity instruments and Bar lists
        catalog.write_data([instrument])
        catalog.write_data(bars)

        written += 1
        elapsed = time.time() - t_start
        rate = written / elapsed
        remaining = (len(tickers) - i) / rate if rate > 0 else 0
        print(
            f"  [{i}/{len(tickers)}] {ticker}: {len(bars)} bars"
            f"  ({elapsed:.0f}s elapsed, ~{remaining:.0f}s remaining)",
            flush=True,
        )

    total_elapsed = time.time() - t_start
    print(
        f"\nDone. {written} tickers written, {skipped} skipped (all-NaN)."
        f" Total time: {total_elapsed:.1f}s"
        f" ({written / total_elapsed:.1f} tickers/s)"
    )
    print(f"Catalog written to: {output_dir.resolve()}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Convert wide ohlcv.parquet to NautilusTrader ParquetDataCatalog",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
examples:
  uv run scripts/build_nt_catalog.py
  uv run scripts/build_nt_catalog.py --profile equities_daily
""",
    )
    parser.add_argument(
        "--profile", default="equities_daily",
        help="Configured dataset profile",
    )
    parser.add_argument(
        "--venue", default="XNYS",
        help="Venue string for instrument IDs (default: XNYS)",
    )
    parser.add_argument(
        "--bar-spec", default="1-DAY-LAST",
        help="Bar specification string (default: 1-DAY-LAST)",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    cfg = load_trading_config()
    profile = cfg.profile(args.profile)
    data_dir = cfg.datasets[profile.dataset]
    output = profile.catalog or (cfg.artifacts["catalog_root"] / profile.name)
    venue = profile.venue or args.venue
    bar_spec = profile.bar_spec or args.bar_spec

    if not data_dir.exists():
        print(f"Error: configured dataset root {data_dir} does not exist", file=sys.stderr)
        sys.exit(1)

    if not (data_dir / "ohlcv.parquet").exists():
        print(f"Error: {data_dir / 'ohlcv.parquet'} not found", file=sys.stderr)
        sys.exit(1)

    output.mkdir(parents=True, exist_ok=True)

    build_catalog(
        data_dir=data_dir,
        output_dir=output,
        venue_str=venue,
        bar_spec=bar_spec,
    )


if __name__ == "__main__":
    main()
