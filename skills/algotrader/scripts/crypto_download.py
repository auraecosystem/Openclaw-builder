#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["httpx"]
# ///
"""
Download historical crypto OHLCV data from data.binance.vision.

Downloads both daily (1d) and 5-minute (5m) kline CSVs for all pairs
in universe.json. Idempotent: skips existing files.

Usage:
    uv run scripts/crypto_download.py --data-dir data-crypto
    uv run scripts/crypto_download.py --data-dir data-crypto --timeframe 1d  # daily only
    uv run scripts/crypto_download.py --data-dir data-crypto --timeframe 5m  # 5m only
"""

import argparse
import asyncio
import io
import json
import sys
import zipfile
from datetime import datetime
from pathlib import Path

import httpx

BASE_URL = "https://data.binance.vision/data/spot/monthly/klines"
MAX_CONCURRENT = 10
# Binance kline CSV columns (no header in the files)
CSV_COLUMNS = [
    "open_time", "open", "high", "low", "close", "volume",
    "close_time", "quote_volume", "trades", "taker_buy_base",
    "taker_buy_quote", "ignore",
]


def generate_months(start_month: str) -> list[tuple[int, int]]:
    """Generate (year, month) tuples from start_month to current month."""
    start = datetime.strptime(start_month, "%Y-%m")
    now = datetime.now()
    months = []
    y, m = start.year, start.month
    while (y, m) <= (now.year, now.month):
        months.append((y, m))
        m += 1
        if m > 12:
            m = 1
            y += 1
    return months


async def download_one(
    client: httpx.AsyncClient,
    sem: asyncio.Semaphore,
    symbol: str,
    timeframe: str,
    year: int,
    month: int,
    raw_dir: Path,
) -> bool:
    """Download and extract one monthly kline zip. Returns True if new data saved."""
    fname = f"{symbol}-{timeframe}-{year}-{month:02d}"
    csv_path = raw_dir / symbol / f"{fname}.csv"
    if csv_path.exists():
        return False

    url = f"{BASE_URL}/{symbol}/{timeframe}/{fname}.zip"
    async with sem:
        try:
            resp = await client.get(url, follow_redirects=True, timeout=30)
            if resp.status_code == 404:
                return False
            resp.raise_for_status()
        except (httpx.HTTPError, httpx.TimeoutException) as e:
            print(f"  WARN: {fname}: {e}", file=sys.stderr)
            return False

    csv_path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with zipfile.ZipFile(io.BytesIO(resp.content)) as zf:
            names = zf.namelist()
            csv_name = next((n for n in names if n.endswith(".csv")), None)
            if csv_name is None:
                return False
            csv_path.write_bytes(zf.read(csv_name))
    except zipfile.BadZipFile:
        print(f"  WARN: {fname}: bad zip", file=sys.stderr)
        return False

    return True


async def download_symbol(
    client: httpx.AsyncClient,
    sem: asyncio.Semaphore,
    symbol: str,
    listing_month: str,
    timeframe: str,
    raw_dir: Path,
) -> int:
    """Download all months for one symbol. Returns count of new files."""
    months = generate_months(listing_month)
    tasks = [
        download_one(client, sem, symbol, timeframe, y, m, raw_dir)
        for y, m in months
    ]
    results = await asyncio.gather(*tasks)
    return sum(1 for r in results if r)


async def run(data_dir: Path, timeframes: list[str]):
    universe_path = data_dir / "universe.json"
    universe = json.loads(universe_path.read_text())
    pairs = universe["pairs"]
    raw_dir = data_dir / "raw"

    sem = asyncio.Semaphore(MAX_CONCURRENT)

    for tf in timeframes:
        print(f"\n=== Downloading {tf} data for {len(pairs)} pairs ===", file=sys.stderr)
        async with httpx.AsyncClient() as client:
            for i, pair in enumerate(pairs, 1):
                symbol = pair["symbol"]
                listing = pair["listing_month"]
                new_count = await download_symbol(client, sem, symbol, listing, tf, raw_dir)
                total = len(generate_months(listing))
                existing = total - new_count
                status = f"({existing}/{total} cached)" if new_count == 0 else f"(+{new_count} new)"
                print(f"  [{i}/{len(pairs)}] {symbol} {tf} {status}", file=sys.stderr)

    print("\nDone.", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description="Download crypto OHLCV from Binance")
    parser.add_argument("--data-dir", required=True, help="Path to data-crypto directory")
    parser.add_argument("--timeframe", default=None, choices=["1d", "5m"],
                        help="Download only this timeframe (default: both)")
    args = parser.parse_args()

    timeframes = [args.timeframe] if args.timeframe else ["1d", "5m"]
    asyncio.run(run(Path(args.data_dir), timeframes))


if __name__ == "__main__":
    main()
