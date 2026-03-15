#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["pandas", "pyarrow", "numpy"]
# ///
"""
Build 5-minute Qullamaggie data: same indicator stack as daily, computed on 5m bars.

Creates data-crypto-5m/ with ohlcv.parquet (5m bars) + cache/ (indicators on 5m).
The engine treats each 5m bar as a "row" — same VCP, flag, RS, regime logic applies.

Usage:
    uv run scripts/crypto_build_5m.py
    uv run scripts/crypto_build_5m.py --profile crypto_5m --indicators
"""

import argparse
import json
import shutil
import sys
from pathlib import Path

_script_dir = Path(__file__).resolve().parent
sys.path.insert(0, str(_script_dir.parent))
sys.path.insert(0, str(_script_dir))

from crypto_build import build_wide_parquet, compute_indicators
from lib import universe as uni
from trading_config import load_trading_config


def _log(msg: str):
    print(f"  {msg}", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description="Build 5m Qullamaggie data")
    parser.add_argument("--profile", default="crypto_5m", help="Configured dataset profile")
    parser.add_argument("--indicators", action="store_true", help="Only recompute indicators")
    args = parser.parse_args()

    cfg = load_trading_config()
    profile = cfg.profile(args.profile)
    src_dir = cfg.datasets[profile.dataset]
    if profile.ohlcv is None:
        raise RuntimeError(f"Profile '{profile.name}' is missing an ohlcv path")
    out_dir = profile.ohlcv.parent
    out_dir.mkdir(parents=True, exist_ok=True)

    # Copy universe.json (needed by engine for ticker list)
    for f in ["universe.json", "etf_tickers.txt"]:
        src = src_dir / f
        if src.exists():
            shutil.copy2(src, out_dir / f)

    ohlcv_path = out_dir / "ohlcv.parquet"

    if not args.indicators:
        # Build 5m wide parquet from raw CSVs
        universe = json.loads((src_dir / "universe.json").read_text())
        pairs = universe["pairs"]
        raw_dir = src_dir / "raw"

        _log("=== Building 5m wide parquet ===")
        build_wide_parquet(raw_dir, pairs, "5m", ohlcv_path)

    if not ohlcv_path.exists():
        print(f"No ohlcv.parquet at {ohlcv_path}. Run without --indicators first.", file=sys.stderr)
        sys.exit(1)

    # Compute indicators on 5m bars
    _log("\n=== Computing indicators on 5m bars ===")
    ohlcv = uni.load_ohlcv(out_dir)
    compute_indicators(out_dir, ohlcv)

    _log("\nDone. Engine data at: " + str(out_dir))


if __name__ == "__main__":
    main()
