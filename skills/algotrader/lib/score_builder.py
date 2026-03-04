"""
Build composite score arrays for the rotation strategy.

Produces three (n_dates, n_tickers) float32 numpy arrays:
- rs_score.npy     — weighted RS percentile rank across 3 timeframes
- pattern_score.npy — fuzzy max(VCP, flag) quality score
- signal_score.npy  — weighted sum of signal algorithm features (requires PyO3 bridge)

Usage:
    python lib/score_builder.py --cache-dir data/cache/
"""

import argparse
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from lib import cache


def build_rs_scores(
    data_dir: Path,
    w_1m: float = 0.2,
    w_3m: float = 0.5,
    w_6m: float = 0.3,
) -> np.ndarray:
    """Weighted combination of RS percentile ranks across 3 horizons.

    Returns (n_dates, n_tickers) float32 array in [0, 1].
    """
    rs_1m = cache.load(data_dir, "rs_pctrank_1m").values.astype(np.float32)
    rs_3m = cache.load(data_dir, "rs_pctrank_3m").values.astype(np.float32)
    rs_6m = cache.load(data_dir, "rs_pctrank_6m").values.astype(np.float32)

    # NaN propagation: if any horizon is NaN, the composite is NaN
    return w_1m * rs_1m + w_3m * rs_3m + w_6m * rs_6m


def build_pattern_scores(data_dir: Path) -> np.ndarray:
    """Fuzzy VCP + flag composite score — max(vcp_score, flag_score).

    VCP score: (num_contractions / 5) * (1 - tightening_ratio), clipped to [0, 1]
    Flag score: pole_pct * (1 - retrace_pct) * (1 - vol_ratio), clipped to [0, 1]

    Returns (n_dates, n_tickers) float32 array in [0, 1].
    """
    # VCP components
    nc = cache.load(data_dir, "vcp_num_contractions").values.astype(np.float32)
    tr = cache.load(data_dir, "vcp_tightening_ratio").values.astype(np.float32)
    vcp_score = np.clip(nc / 5.0, 0, 1) * np.clip(1 - tr, 0, 1)

    # Flag components
    pp = cache.load(data_dir, "flag_pole_pct").values.astype(np.float32)
    rp = cache.load(data_dir, "flag_retrace_pct").values.astype(np.float32)
    vr = cache.load(data_dir, "flag_vol_ratio").values.astype(np.float32)
    flag_score = np.clip(pp, 0, 0.5) * 2 * np.clip(1 - rp, 0, 1) * np.clip(1 - vr, 0, 1)

    # Best of two patterns; NaN → 0 for safe max
    vcp_safe = np.nan_to_num(vcp_score, nan=0.0)
    flag_safe = np.nan_to_num(flag_score, nan=0.0)
    return np.maximum(vcp_safe, flag_safe).astype(np.float32)


def compute_signal_features(data_dir: Path) -> np.ndarray | None:
    """Run the Rust signal pipeline via PyO3 on all tickers.

    Loads close prices from ohlcv.parquet, processes each ticker through
    the 18-algorithm signal pipeline, and returns (n_dates, n_tickers, 13)
    normalized feature array.

    Requires: `maturin develop --features python` in engine/.
    Returns None if the PyO3 bridge is not installed.
    """
    try:
        from algotrader_engine import SignalEngine
    except ImportError:
        return None

    from lib.universe import load_ohlcv

    ohlcv = load_ohlcv(data_dir)
    close_df = ohlcv["close"]
    volume_df = ohlcv["volume"]
    tickers = list(close_df.columns)

    # Build per-ticker close/volume lists (SignalEngine expects list[list[float]])
    close_matrix = []
    volume_matrix = []
    for ticker in tickers:
        c = close_df[ticker].values.astype(np.float32)
        v = volume_df[ticker].values.astype(np.float32)
        # Replace NaN with 0 for the Rust pipeline
        c = np.nan_to_num(c, nan=0.0)
        v = np.nan_to_num(v, nan=0.0)
        close_matrix.append(c.tolist())
        volume_matrix.append(v.tolist())

    print(f"  Running signal pipeline on {len(tickers)} tickers...")
    engine = SignalEngine()
    # returns (n_tickers, n_bars, 13)
    raw = engine.run_batch(close_matrix, volume_matrix)

    # Convert to numpy and transpose to (n_bars, n_tickers, 13)
    arr = np.array(raw, dtype=np.float32)  # (n_tickers, n_bars, 13)
    return arr.transpose(1, 0, 2)  # (n_bars, n_tickers, 13)


def build_signal_scores(
    data_dir: Path,
    weights: dict[str, float] | None = None,
) -> np.ndarray | None:
    """Weighted sum of 13 signal algorithm features.

    First tries to load pre-computed signal arrays from cache. If not found,
    runs the PyO3 signal pipeline to compute them. Returns (n_dates, n_tickers)
    float32 array, or None if neither cached arrays nor PyO3 bridge are available.
    """
    cache_dir = Path(data_dir) / "cache"

    # Try loading pre-computed features
    features_path = cache_dir / "signal_features.npy"
    if features_path.exists():
        stacked = np.load(features_path)  # (n_dates, n_tickers, 13)
    else:
        # Compute via PyO3
        stacked = compute_signal_features(data_dir)
        if stacked is None:
            return None
        np.save(features_path, stacked)
        print(f"  Saved signal features: {stacked.shape}")

    # Feature names for weight mapping
    try:
        from algotrader_engine import SignalEngine
        feature_names = SignalEngine.feature_names()
    except ImportError:
        feature_names = [
            "perm_entropy", "bocpd_prob", "kalman_level", "kalman_trend",
            "vmd_sta_lta", "knn_anomaly", "scatter_class", "vpin",
            "mp_novelty", "template_corr", "swt_denoised", "hurst", "hmm_state",
        ]

    # Default: equal weights
    n_features = stacked.shape[-1]
    if weights is None:
        w = np.ones(n_features, dtype=np.float32) / n_features
    else:
        w = np.array(
            [weights.get(n, 1.0 / n_features) for n in feature_names],
            dtype=np.float32,
        )

    # Weighted sum across features → (n_dates, n_tickers)
    return (stacked * w).sum(axis=-1).astype(np.float32)


def build_all_scores(data_dir: Path) -> dict[str, np.ndarray]:
    """Build all available composite scores."""
    cache_dir = Path(data_dir) / "cache"
    scores = {}

    print("Building RS scores...")
    scores["rs_score"] = build_rs_scores(data_dir)
    np.save(cache_dir / "rs_score.npy", scores["rs_score"])

    print("Building pattern scores...")
    scores["pattern_score"] = build_pattern_scores(data_dir)
    np.save(cache_dir / "pattern_score.npy", scores["pattern_score"])

    print("Building signal scores...")
    sig = build_signal_scores(data_dir)
    if sig is not None:
        scores["signal_score"] = sig
        np.save(cache_dir / "signal_score.npy", sig)
        print("  Signal scores built.")
    else:
        print("  Signal arrays not found — skipping (run Phase 1 PyO3 bridge first).")

    return scores


def main():
    parser = argparse.ArgumentParser(description="Build composite score arrays")
    parser.add_argument("--data-dir", required=True, help="Path to data directory")
    args = parser.parse_args()
    build_all_scores(Path(args.data_dir))


if __name__ == "__main__":
    main()
