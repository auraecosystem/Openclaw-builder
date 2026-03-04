"""
Build all 26 indicator caches from ohlcv.parquet.

Consolidates computation of OHLCV-derived indicators (11), cross-sectional
RS ranks (3), pattern intermediates (8), derived ratios (2), and consolidation
high/pct_10d (2) into a single script. Output: per-indicator float32 ZSTD
parquet files in <data_dir>/cache/.

Usage:
    python lib/build_cache.py --data-dir data/
    python lib/build_cache.py --data-dir data-crypto/ --force
"""

import argparse
import sys
import time
from pathlib import Path

import pandas as pd

# Add parent dir to path so lib imports work when run as script
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from lib import cache, indicators as ind, patterns, universe


# All 26 indicator cache keys
INDICATOR_KEYS = [
    # OHLCV-derived (11)
    "atr_14", "sma_10", "sma_20", "vol_sma_20",
    "ret_21", "ret_63", "ret_126",
    "pct_10d", "dist_52w", "consol_high", "consec_green",
    # Cross-sectional RS ranks (3)
    "rs_pctrank_1m", "rs_pctrank_3m", "rs_pctrank_6m",
    # VCP intermediates (4)
    "vcp_num_contractions", "vcp_last_contraction_pct",
    "vcp_tightening_ratio", "vcp_vol_trend",
    # Flag intermediates (4)
    "flag_pole_pct", "flag_retrace_pct", "flag_days", "flag_vol_ratio",
    # Derived ratios (2)
    "adr_pct", "extension_atr",
]


def build_ohlcv_indicators(
    ohlcv: dict[str, pd.DataFrame],
) -> dict[str, pd.DataFrame]:
    """Compute the 11 OHLCV-derived indicators."""
    high, low, close, volume = ohlcv["high"], ohlcv["low"], ohlcv["close"], ohlcv["volume"]
    open_ = ohlcv["open"]

    atr_14 = ind.atr(high, low, close, period=14)
    sma_10 = ind.sma(close, 10)
    sma_20 = ind.sma(close, 20)
    vol_sma_20 = ind.volume_sma(volume, 20)

    rets = ind.rolling_return(close, [21, 63, 126])
    ret_21, ret_63, ret_126 = rets[21], rets[63], rets[126]

    pct_10d = ind.pct_change_n_days(close, 10)
    dist_52w = ind.distance_from_52w_high(close, high)
    consol_high = patterns.consolidation_high(high, lookback=40)
    consec_green = ind.consecutive_green_days(open_, close)

    return {
        "atr_14": atr_14,
        "sma_10": sma_10,
        "sma_20": sma_20,
        "vol_sma_20": vol_sma_20,
        "ret_21": ret_21,
        "ret_63": ret_63,
        "ret_126": ret_126,
        "pct_10d": pct_10d,
        "dist_52w": dist_52w,
        "consol_high": consol_high,
        "consec_green": consec_green,
    }


def build_rs_pctranks(
    rets: dict[str, pd.DataFrame],
) -> dict[str, pd.DataFrame]:
    """Compute cross-sectional RS percentile ranks (3 timeframes)."""
    returns = {21: rets["ret_21"], 63: rets["ret_63"], 126: rets["ret_126"]}
    ranks = universe.rs_percentile_ranks(returns)
    return {
        "rs_pctrank_1m": ranks["pctrank_1m"],
        "rs_pctrank_3m": ranks["pctrank_3m"],
        "rs_pctrank_6m": ranks["pctrank_6m"],
    }


def build_pattern_intermediates(
    ohlcv: dict[str, pd.DataFrame],
) -> dict[str, pd.DataFrame]:
    """Compute VCP + flag pattern intermediates (8 total)."""
    high, low, close, volume = ohlcv["high"], ohlcv["low"], ohlcv["close"], ohlcv["volume"]

    print("  Computing VCP intermediates (numba)...")
    vcp = patterns.vcp_intermediates(high, low, close, volume)

    print("  Computing flag intermediates (numba)...")
    flag = patterns.flag_intermediates(high, low, close, volume)

    return {
        "vcp_num_contractions": vcp["num_contractions"],
        "vcp_last_contraction_pct": vcp["last_contraction_pct"],
        "vcp_tightening_ratio": vcp["tightening_ratio"],
        "vcp_vol_trend": vcp["vol_trend"],
        "flag_pole_pct": flag["pole_pct"],
        "flag_retrace_pct": flag["retrace_pct"],
        "flag_days": flag["days"],
        "flag_vol_ratio": flag["vol_ratio"],
    }


def build_derived(
    indicators: dict[str, pd.DataFrame],
) -> dict[str, pd.DataFrame]:
    """Compute derived ratios: AdrPct and ExtensionAtr."""
    return {
        "adr_pct": ind.adr_pct(indicators["atr_14"], indicators.get("close", indicators["atr_14"])),
        "extension_atr": ind.extension_atr(
            indicators.get("close", indicators["atr_14"]),
            indicators["consol_high"],
            indicators["atr_14"],
        ),
    }


def build_all(data_dir: Path, force: bool = False) -> None:
    """Build all 26 indicator caches."""
    if not force and cache.is_valid(data_dir, required_keys=INDICATOR_KEYS):
        print(f"Cache is valid and complete in {data_dir}/cache/. Use --force to rebuild.")
        return

    t0 = time.time()

    # Load OHLCV
    print(f"Loading OHLCV from {data_dir}...")
    ohlcv = universe.load_ohlcv(data_dir)
    shape = ohlcv["close"].shape
    print(f"  Shape: {shape[0]} rows x {shape[1]} tickers")

    # Phase 1: OHLCV indicators
    print("Computing OHLCV indicators...")
    t1 = time.time()
    ohlcv_ind = build_ohlcv_indicators(ohlcv)
    print(f"  Done in {time.time() - t1:.1f}s")

    # Phase 2: RS percentile ranks
    print("Computing RS percentile ranks...")
    t1 = time.time()
    rs = build_rs_pctranks(ohlcv_ind)
    print(f"  Done in {time.time() - t1:.1f}s")

    # Phase 3: Pattern intermediates (slowest — numba JIT)
    print("Computing pattern intermediates...")
    t1 = time.time()
    pats = build_pattern_intermediates(ohlcv)
    print(f"  Done in {time.time() - t1:.1f}s")

    # Phase 4: Derived ratios
    print("Computing derived ratios...")
    # build_derived needs close for adr_pct and extension_atr
    ohlcv_ind_with_close = {**ohlcv_ind, "close": ohlcv["close"]}
    derived = build_derived(ohlcv_ind_with_close)

    # Merge all and write
    all_indicators = {**ohlcv_ind, **rs, **pats, **derived}

    print(f"Writing {len(all_indicators)} indicators to cache...")
    for name, df in all_indicators.items():
        cache.save(data_dir, name, df)
    cache.seal(data_dir)

    elapsed = time.time() - t0
    print(f"Cache build complete in {elapsed:.1f}s ({len(all_indicators)} indicators)")


def main():
    parser = argparse.ArgumentParser(description="Build all 26 indicator caches")
    parser.add_argument("--data-dir", required=True, help="Path to data directory with ohlcv.parquet")
    parser.add_argument("--force", action="store_true", help="Rebuild even if cache is valid")
    args = parser.parse_args()

    build_all(Path(args.data_dir), force=args.force)


if __name__ == "__main__":
    main()
