#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["pandas", "pyarrow", "numpy"]
# ///
"""
Build crypto parquets and compute indicators from downloaded Binance CSVs.

Three phases:
  A) Build ohlcv_daily.parquet — wide MultiIndex, daily bars
  B) Build ohlcv_5m.parquet   — wide MultiIndex, 5-minute bars
  C) Compute indicators from daily bars and cache them

Usage:
    uv run scripts/crypto_build.py --data-dir data-crypto
    uv run scripts/crypto_build.py --data-dir data-crypto --skip-5m      # skip 5m parquet
    uv run scripts/crypto_build.py --data-dir data-crypto --indicators   # only recompute indicators
"""

import argparse
import json
import sys
import time
from pathlib import Path

import numpy as np
import pandas as pd

# Add parent dir so we can import from lib/
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from lib import cache as _cache
from lib import indicators as ind
from lib import patterns as pat
from lib import universe as uni


def _log(msg: str):
    print(f"  {msg}", file=sys.stderr)


def load_symbol_csv(raw_dir: Path, symbol: str, timeframe: str) -> pd.DataFrame | None:
    """Load and concat all monthly CSVs for one symbol/timeframe."""
    sym_dir = raw_dir / symbol
    pattern = f"{symbol}-{timeframe}-*.csv"
    files = sorted(sym_dir.glob(pattern))
    if not files:
        return None

    # Binance kline CSVs: 12 columns, no header
    col_names = [
        "open_time", "open", "high", "low", "close", "volume",
        "close_time", "quote_volume", "trades", "taker_buy_base",
        "taker_buy_quote", "ignore",
    ]

    dfs = []
    for f in files:
        try:
            df = pd.read_csv(f, header=None, names=col_names)
            dfs.append(df)
        except Exception as e:
            _log(f"WARN: {f.name}: {e}")
            continue

    if not dfs:
        return None

    combined = pd.concat(dfs, ignore_index=True)
    # Binance changed timestamp format: older files use ms (13 digits), newer use us (16 digits)
    # Normalize to milliseconds
    ts = combined["open_time"].copy()
    ts = ts.where(ts < 1e13, ts // 1000)  # if > 1e13, it's microseconds → convert to ms
    combined["timestamp"] = pd.to_datetime(ts, unit="ms", utc=True)
    combined = combined.set_index("timestamp").sort_index()
    combined = combined[~combined.index.duplicated(keep="first")]

    # Keep only OHLCV, cast to float32
    ohlcv = combined[["open", "high", "low", "close", "volume"]].astype(np.float32)
    return ohlcv


def build_wide_parquet(
    raw_dir: Path,
    pairs: list[dict],
    timeframe: str,
    output_path: Path,
) -> pd.DataFrame:
    """Build a wide MultiIndex parquet from per-symbol CSVs."""
    t0 = time.time()
    all_dfs = {}
    for i, pair in enumerate(pairs, 1):
        symbol = pair["symbol"]
        df = load_symbol_csv(raw_dir, symbol, timeframe)
        if df is not None:
            all_dfs[symbol] = df
            _log(f"[{i}/{len(pairs)}] {symbol}: {len(df)} rows")
        else:
            _log(f"[{i}/{len(pairs)}] {symbol}: no data")

    if not all_dfs:
        raise RuntimeError(f"No {timeframe} data found in {raw_dir}")

    # Build wide DataFrame with MultiIndex columns (field, symbol)
    fields = ["open", "high", "low", "close", "volume"]
    wide_parts = {}
    for field in fields:
        field_dfs = {sym: df[field] for sym, df in all_dfs.items()}
        wide_parts[field] = pd.DataFrame(field_dfs)

    wide = pd.concat(wide_parts, axis=1)  # MultiIndex: (field, symbol)
    wide = wide.sort_index()

    # For daily: convert to date-only index (strip time/tz)
    if timeframe == "1d":
        wide.index = wide.index.normalize().tz_localize(None)
        # Drop duplicate dates (can happen at month boundaries)
        wide = wide[~wide.index.duplicated(keep="first")]

    _log(f"Wide shape: {wide.shape[0]} rows × {wide.shape[1]} cols")
    _log(f"Date range: {wide.index[0]} → {wide.index[-1]}")

    output_path.parent.mkdir(parents=True, exist_ok=True)
    wide.to_parquet(output_path, compression="zstd")
    elapsed = time.time() - t0
    _log(f"Saved {output_path} ({output_path.stat().st_size / 1e6:.1f} MB) in {elapsed:.1f}s")
    return wide


def compute_indicators(data_dir: Path, ohlcv: dict[str, pd.DataFrame]):
    """Compute all indicators from daily OHLCV and save to cache."""
    close = ohlcv["close"]
    high = ohlcv["high"]
    low = ohlcv["low"]
    open_ = ohlcv["open"]
    volume = ohlcv["volume"]

    _log("Computing basic indicators...")
    atr_14 = ind.atr(high, low, close, period=14)
    sma_10 = ind.sma(close, 10)
    sma_20 = ind.sma(close, 20)
    vol_sma_20 = ind.volume_sma(volume, 20)
    returns = ind.rolling_return(close, periods=[21, 63, 126])
    dist_52w = ind.distance_from_52w_high(close, high)
    pct_10d = ind.pct_change_n_days(close, 10)
    consec_green = ind.consecutive_green_days(open_, close)

    _log("Computing RS percentile ranks...")
    rs_ranks = uni.rs_percentile_ranks(returns)

    _log("Computing pattern intermediates...")
    vcp_pre = pat.vcp_intermediates(high, low, close, volume)
    flag_pre = pat.flag_intermediates(high, low, close, volume)
    consol_high_df = pat.consolidation_high(high)

    _log("Saving to cache...")
    _cache.save(data_dir, "atr_14", atr_14)
    _cache.save(data_dir, "sma_10", sma_10)
    _cache.save(data_dir, "sma_20", sma_20)
    _cache.save(data_dir, "vol_sma_20", vol_sma_20)
    for period, df in returns.items():
        _cache.save(data_dir, f"ret_{period}", df)
    _cache.save(data_dir, "dist_52w", dist_52w)
    _cache.save(data_dir, "pct_10d", pct_10d)
    _cache.save(data_dir, "consec_green", consec_green)
    _cache.save(data_dir, "rs_pctrank_1m", rs_ranks["pctrank_1m"])
    _cache.save(data_dir, "rs_pctrank_3m", rs_ranks["pctrank_3m"])
    _cache.save(data_dir, "rs_pctrank_6m", rs_ranks["pctrank_6m"])
    _cache.save(data_dir, "vcp_num_contractions", vcp_pre["num_contractions"])
    _cache.save(data_dir, "vcp_last_contraction_pct", vcp_pre["last_contraction_pct"])
    _cache.save(data_dir, "vcp_tightening_ratio", vcp_pre["tightening_ratio"])
    _cache.save(data_dir, "vcp_vol_trend", vcp_pre["vol_trend"])
    _cache.save(data_dir, "flag_pole_pct", flag_pre["pole_pct"])
    _cache.save(data_dir, "flag_retrace_pct", flag_pre["retrace_pct"])
    _cache.save(data_dir, "flag_days", flag_pre["days"])
    _cache.save(data_dir, "flag_vol_ratio", flag_pre["vol_ratio"])
    _cache.save(data_dir, "consol_high", consol_high_df)
    _cache.seal(data_dir)
    _log("Indicators cached")


def main():
    parser = argparse.ArgumentParser(description="Build crypto parquets + indicators")
    parser.add_argument("--data-dir", required=True, help="Path to data-crypto directory")
    parser.add_argument("--skip-5m", action="store_true", help="Skip building 5m parquet")
    parser.add_argument("--indicators", action="store_true", help="Only recompute indicators")
    args = parser.parse_args()

    data_dir = Path(args.data_dir)
    universe = json.loads((data_dir / "universe.json").read_text())
    pairs = universe["pairs"]
    raw_dir = data_dir / "raw"

    if args.indicators:
        _log("Loading existing daily parquet for indicator computation...")
        ohlcv_path = data_dir / "ohlcv_daily.parquet"
        # Also check for ohlcv.parquet (the name the cache module expects)
        if not (data_dir / "ohlcv.parquet").exists() and ohlcv_path.exists():
            import shutil
            shutil.copy2(ohlcv_path, data_dir / "ohlcv.parquet")
        ohlcv = uni.load_ohlcv(data_dir)
        compute_indicators(data_dir, ohlcv)
        return

    # Phase A: Daily parquet
    _log("=== Phase A: Building daily parquet ===")
    daily_path = data_dir / "ohlcv_daily.parquet"
    build_wide_parquet(raw_dir, pairs, "1d", daily_path)

    # The cache module looks for ohlcv.parquet — symlink or copy
    ohlcv_link = data_dir / "ohlcv.parquet"
    if not ohlcv_link.exists():
        ohlcv_link.symlink_to("ohlcv_daily.parquet")
        _log("Symlinked ohlcv.parquet → ohlcv_daily.parquet")

    # Phase B: 5m parquet
    if not args.skip_5m:
        _log("\n=== Phase B: Building 5m parquet ===")
        fivemin_path = data_dir / "ohlcv_5m.parquet"
        build_wide_parquet(raw_dir, pairs, "5m", fivemin_path)

    # Phase C: Indicators from daily bars
    _log("\n=== Phase C: Computing indicators from daily bars ===")
    ohlcv = uni.load_ohlcv(data_dir)
    compute_indicators(data_dir, ohlcv)

    _log("\nAll done.")


if __name__ == "__main__":
    main()
