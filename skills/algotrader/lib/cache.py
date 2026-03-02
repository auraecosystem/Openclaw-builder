"""
Indicator cache backed by per-DataFrame parquet files.

Cache lives in <data_dir>/cache/. It is invalidated whenever ohlcv.parquet
changes (mtime comparison stored in meta.json).

Usage:
    if cache.is_valid(data_dir):
        atr_14 = cache.load(data_dir, "atr_14")
    else:
        atr_14 = ind.atr(...)
        cache.save(data_dir, "atr_14", atr_14)
        cache.seal(data_dir)   # mark cache valid after all saves
"""

import json
import time
from pathlib import Path

import pandas as pd


def _cache_dir(data_dir: Path) -> Path:
    return Path(data_dir) / "cache"


def _meta_path(data_dir: Path) -> Path:
    return _cache_dir(data_dir) / "meta.json"


def _ohlcv_mtime(data_dir: Path) -> float:
    return (Path(data_dir) / "ohlcv.parquet").stat().st_mtime


def is_valid(data_dir: Path, required_keys: list[str] | None = None) -> bool:
    """True if cache exists, matches current ohlcv.parquet mtime, and all required keys are present."""
    meta = _meta_path(data_dir)
    if not meta.exists():
        return False
    try:
        m = json.loads(meta.read_text())
        if m.get("ohlcv_mtime") != _ohlcv_mtime(data_dir):
            return False
        if required_keys:
            d = _cache_dir(data_dir)
            if any(not (d / f"{k}.parquet").exists() for k in required_keys):
                return False
        return True
    except Exception:
        return False


def save(data_dir: Path, name: str, df: pd.DataFrame) -> None:
    """Write a wide DataFrame to the cache (zstd parquet, float32)."""
    d = _cache_dir(data_dir)
    d.mkdir(parents=True, exist_ok=True)
    # Cast float64 columns to float32 — indicators don't need double precision,
    # and this halves storage vs the pandas/numpy default of float64.
    df = df.astype({c: "float32" for c, t in df.dtypes.items() if t == "float64"})
    df.to_parquet(d / f"{name}.parquet", compression="zstd")


def load(data_dir: Path, name: str) -> pd.DataFrame:
    """Read a cached wide DataFrame."""
    return pd.read_parquet(_cache_dir(data_dir) / f"{name}.parquet")


def seal(data_dir: Path) -> None:
    """Write meta.json to mark the cache as consistent."""
    meta = {
        "ohlcv_mtime": _ohlcv_mtime(data_dir),
        "created_at": time.time(),
    }
    _meta_path(data_dir).write_text(json.dumps(meta))


def invalidate(data_dir: Path) -> None:
    """Remove meta.json so the next run rebuilds the cache."""
    p = _meta_path(data_dir)
    if p.exists():
        p.unlink()
