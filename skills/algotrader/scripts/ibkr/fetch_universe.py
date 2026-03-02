# /// script
# requires-python = ">=3.12"
# dependencies = ["ib_async", "pandas", "pyarrow"]
# ///
"""Fetch 10 years of hourly bars for the US equity universe.

Handles IBKR's 5-year max duration for hourly bars by making two requests
per symbol (5Y ago → now, 10Y ago → 5Y ago) and concatenating.
Also handles VIX as an Index contract.

Usage:
    uv run scripts/ibkr/fetch_universe.py
    uv run scripts/ibkr/fetch_universe.py --port 4002   # paper
    uv run scripts/ibkr/fetch_universe.py --years 5      # just 5 years
"""

from __future__ import annotations

import argparse
import sys
import time
from datetime import datetime, timezone

import pandas as pd
from ib_async import IB, Index, Stock, util


UNIVERSE = [
    "NVDA", "AAPL", "MSFT", "META", "AVGO", "CRM", "NFLX", "PLTR",
    "GS", "CAT", "GE", "UBER",
    "LLY", "UNH", "ISRG",
    "COST", "CMG", "COIN",
    "SPY", "QQQ",
    "VIX",
]

INDEX_SYMBOLS = {"VIX"}


def make_contract(symbol: str):
    if symbol in INDEX_SYMBOLS:
        return Index(symbol, "CBOE", "USD")
    return Stock(symbol, "SMART", "USD")


def fetch_bars(ib: IB, symbol: str, end_dt: str, duration: str, bar_size: str) -> pd.DataFrame | None:
    contract = make_contract(symbol)
    try:
        bars = ib.reqHistoricalData(
            contract,
            endDateTime=end_dt,
            durationStr=duration,
            barSizeSetting=bar_size,
            whatToShow="TRADES",
            useRTH=True,
            keepUpToDate=False,
        )
    except Exception as e:
        print(f"    error: {e}", file=sys.stderr)
        return None

    if not bars:
        return None

    df = util.df(bars)
    df.insert(0, "symbol", symbol)
    return df


def main():
    p = argparse.ArgumentParser(description="Fetch US equity universe hourly bars")
    p.add_argument("--host", default="127.0.0.1")
    p.add_argument("--port", type=int, default=7496)
    p.add_argument("--client-id", type=int, default=20)
    p.add_argument("--years", type=int, default=10, help="Total years of data (default: 10)")
    p.add_argument("--bar-size", default="1 hour")
    p.add_argument("-o", "--output", default="data-ibkr/universe_hourly.parquet")
    args = p.parse_args()

    ib = IB()
    print(f"Connecting to {args.host}:{args.port} ...", file=sys.stderr)
    ib.connect(args.host, args.port, clientId=args.client_id)
    print("Connected.\n", file=sys.stderr)

    # For hourly bars, IBKR max duration is ~5Y per request.
    # Split into 5Y chunks working backward from now.
    chunk_years = 5
    now = datetime.now(timezone.utc)
    chunks = []
    remaining = args.years
    cursor = ""  # empty = now
    while remaining > 0:
        yrs = min(remaining, chunk_years)
        chunks.append((f"{yrs} Y", cursor))
        # Move cursor back
        cursor_dt = (now if not cursor else datetime.strptime(cursor, "%Y%m%d-%H:%M:%S"))
        cursor_dt = cursor_dt.replace(year=cursor_dt.year - yrs)
        cursor = cursor_dt.strftime("%Y%m%d-%H:%M:%S")
        remaining -= yrs

    all_frames: list[pd.DataFrame] = []

    for i, symbol in enumerate(UNIVERSE):
        print(f"[{i+1}/{len(UNIVERSE)}] {symbol}", file=sys.stderr, end="", flush=True)
        symbol_frames = []

        for chunk_dur, chunk_end in chunks:
            df = fetch_bars(ib, symbol, chunk_end, chunk_dur, args.bar_size)
            if df is not None and not df.empty:
                symbol_frames.append(df)
                print(f" +{len(df)}", file=sys.stderr, end="", flush=True)
            else:
                print(f" (no data for {chunk_dur} ending {chunk_end or 'now'})", file=sys.stderr, end="", flush=True)
            # Respect pacing: 1s between requests
            time.sleep(1)

        if symbol_frames:
            combined = pd.concat(symbol_frames, ignore_index=True)
            combined = combined.drop_duplicates(subset=["symbol", "date"]).sort_values("date")
            all_frames.append(combined)
            print(f" = {len(combined)} bars total", file=sys.stderr)
        else:
            print(f" = NO DATA", file=sys.stderr)

        # Extra sleep between symbols
        if i < len(UNIVERSE) - 1:
            time.sleep(1)

    ib.disconnect()

    if not all_frames:
        print("No data retrieved.", file=sys.stderr)
        sys.exit(1)

    result = pd.concat(all_frames, ignore_index=True)

    from pathlib import Path
    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    result.to_parquet(out, index=False)

    symbols_ok = result["symbol"].nunique()
    print(f"\nSaved {len(result)} bars ({symbols_ok} symbols) to {out}", file=sys.stderr)

    # Summary per symbol
    for sym, grp in result.groupby("symbol"):
        print(f"  {sym:6s}: {len(grp):>6d} bars  ({grp['date'].iloc[0]} → {grp['date'].iloc[-1]})", file=sys.stderr)


if __name__ == "__main__":
    main()
