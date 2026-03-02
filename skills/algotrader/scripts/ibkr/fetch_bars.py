# /// script
# requires-python = ">=3.12"
# dependencies = ["ib_async", "pandas", "pyarrow"]
# ///
"""Fetch historical bars from IBKR and save to parquet/CSV.

Connects via ib_async (community fork of ib_insync) using reqHistoricalData.
Historical bar requests work WITHOUT a market data subscription.

Prerequisites:
    - TWS Classic or IB Gateway running with API enabled
    - TWS: Edit > Global Configuration > API > Settings > Enable ActiveX and Socket Clients
    - IB Gateway: API enabled by default
    - IBKR Desktop does NOT support TWS API — use TWS Classic or IB Gateway

Ports:
    TWS live=7496, TWS paper=7497, Gateway live=4001, Gateway paper=4002

Rate limits:
    - 60 historical requests per 10 minutes
    - Max 6 requests per 2 seconds for the same contract
    - 15-second cooldown for identical request parameters
    - 1-second sleep between different symbols (built into this script)

Examples:
    uv run scripts/ibkr/fetch_bars.py SPY
    uv run scripts/ibkr/fetch_bars.py SPY AAPL MSFT -b "1 hour" -d "90 D" -o data/hourly.parquet
    uv run scripts/ibkr/fetch_bars.py SPY -b "5 mins" -d "5 D" -o spy_5m.csv
    uv run scripts/ibkr/fetch_bars.py SPY --port 7497  # TWS paper

Troubleshooting:
    ConnectionRefusedError  -> TWS/Gateway not running, or API not enabled, or wrong port
    "No data returned"      -> Symbol not found or no market data for that duration/bar-size
    Pacing violation (162)  -> Too many requests; wait 10s between retries
    ModuleNotFoundError     -> Wrong Python; use the venv with ib_async installed
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

import pandas as pd
from ib_async import IB, Stock, util


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Fetch IBKR historical bars",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
examples:
  uv run scripts/ibkr/fetch_bars.py SPY                              # daily bars, 1 year, print
  uv run scripts/ibkr/fetch_bars.py SPY AAPL -b "1 hour" -d "90 D"  # hourly, 90 days
  uv run scripts/ibkr/fetch_bars.py SPY -o data/spy.parquet          # save parquet
  uv run scripts/ibkr/fetch_bars.py SPY -b "5 mins" -d "5 D" -o x.csv
  uv run scripts/ibkr/fetch_bars.py SPY AAPL QQQ --wide -o wide.parquet

rate limits:
  - 60 historical requests per 10 minutes
  - Max 6 requests per 2 seconds for the same contract
  - 15-second cooldown for identical request parameters
  - Script adds 1s sleep between different symbols automatically
""",
    )
    p.add_argument("symbols", nargs="+", help="Ticker symbols (e.g. SPY AAPL QQQ)")
    p.add_argument("--host", default="127.0.0.1")
    p.add_argument("--port", type=int, default=7496, help="7496=TWS live, 7497=TWS paper, 4001/4002=Gateway")
    p.add_argument("--client-id", type=int, default=20)
    p.add_argument("-d", "--duration", default="1 Y", help="e.g. '1 Y', '6 M', '90 D', '5 D'")
    p.add_argument("-b", "--bar-size", default="1 day", help="e.g. '1 day', '1 hour', '5 mins', '1 min'")
    p.add_argument("-w", "--what", default="TRADES", help="TRADES, MIDPOINT, BID, ASK, ADJUSTED_LAST")
    p.add_argument("--rth", action="store_true", default=True, help="Regular trading hours only (default)")
    p.add_argument("--eth", action="store_true", help="Include extended trading hours")
    p.add_argument("-o", "--output", type=Path, help="Output path (.parquet or .csv). Omit to print.")
    p.add_argument("--wide", action="store_true", help="Wide format (multi-symbol): columns like (open, SPY)")
    return p.parse_args()


def fetch_symbol(ib: IB, symbol: str, args: argparse.Namespace) -> pd.DataFrame | None:
    """Fetch bars for one symbol. Returns DataFrame or None on failure."""
    contract = Stock(symbol, "SMART", "USD")

    try:
        bars = ib.reqHistoricalData(
            contract,
            endDateTime="",
            durationStr=args.duration,
            barSizeSetting=args.bar_size,
            whatToShow=args.what,
            useRTH=not args.eth,
            keepUpToDate=False,
        )
    except Exception as e:
        print(f"  {symbol}: error — {e}", file=sys.stderr)
        return None

    if not bars:
        print(f"  {symbol}: no data returned", file=sys.stderr)
        return None

    df = util.df(bars)
    df.insert(0, "symbol", symbol)
    return df


def main():
    args = parse_args()

    ib = IB()
    print(f"Connecting to {args.host}:{args.port} ...", file=sys.stderr)
    ib.connect(args.host, args.port, clientId=args.client_id)
    print("Connected.\n", file=sys.stderr)

    frames: list[pd.DataFrame] = []
    for i, symbol in enumerate(args.symbols):
        print(f"  [{i+1}/{len(args.symbols)}] {symbol} ...", file=sys.stderr, end=" ")
        df = fetch_symbol(ib, symbol, args)
        if df is not None:
            print(f"{len(df)} bars", file=sys.stderr)
            frames.append(df)
        # Respect pacing: 1s between requests for different symbols
        if i < len(args.symbols) - 1:
            time.sleep(1)

    ib.disconnect()

    if not frames:
        print("No data retrieved.", file=sys.stderr)
        sys.exit(1)

    combined = pd.concat(frames, ignore_index=True)

    # Wide format: pivot so columns are (field, symbol) MultiIndex
    if args.wide and len(args.symbols) > 1:
        combined["date"] = pd.to_datetime(combined["date"])
        combined = combined.pivot(index="date", columns="symbol", values=["open", "high", "low", "close", "volume"])
        combined.columns = [f"{field}_{sym}" for field, sym in combined.columns]

    # Output
    if args.output is None:
        print(combined.to_string(max_rows=40))
        print(f"\n{len(combined)} rows × {len(combined.columns)} cols", file=sys.stderr)
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        if args.output.suffix == ".csv":
            combined.to_csv(args.output, index=True)
        else:
            combined.to_parquet(args.output, index=True)
        print(f"\nSaved {len(combined)} rows to {args.output}", file=sys.stderr)


if __name__ == "__main__":
    main()
