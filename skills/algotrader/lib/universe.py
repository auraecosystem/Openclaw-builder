"""
Universe filtering and ranking.

All filter functions return wide boolean DataFrames (DatetimeIndex × tickers)
where True = ticker qualifies on that date. Composing filters is just boolean AND/OR.
"""

from pathlib import Path

import pandas as pd
import numpy as np


def load_ohlcv(data_dir: Path) -> dict[str, pd.DataFrame]:
    """Load ohlcv.parquet and return dict of wide DataFrames keyed by field.

    The parquet has MultiIndex columns (field, ticker) where
    field ∈ {open, high, low, close, volume}.

    Returns: {'open': df, 'high': df, 'low': df, 'close': df, 'volume': df}
    Each DataFrame: DatetimeIndex × tickers, float32.
    """
    path = Path(data_dir) / "ohlcv.parquet"
    df = pd.read_parquet(path)

    # Handle both MultiIndex columns (field, ticker) and flat columns
    if isinstance(df.columns, pd.MultiIndex):
        return {field: df[field].astype(np.float32) for field in df.columns.get_level_values(0).unique()}

    raise ValueError(f"Expected MultiIndex columns in {path}, got: {df.columns[:5]}")


def load_etf_tickers(data_dir: Path) -> set[str]:
    """Read the list of ETF ticker symbols to exclude from strategies."""
    path = Path(data_dir) / "etf_tickers.txt"
    if not path.exists():
        return set()
    return set(path.read_text().strip().splitlines())


def mask_liquid(
    close: pd.DataFrame,
    volume: pd.DataFrame,
    vol_sma: pd.DataFrame,
    min_price: float = 5.0,
    min_vol: int = 300_000,
) -> pd.DataFrame:
    """Boolean mask: True where price > min_price AND avg volume > min_vol."""
    return (close > min_price) & (vol_sma > min_vol)


def mask_stocks_only(df: pd.DataFrame, etf_tickers: set[str]) -> pd.DataFrame:
    """Boolean mask: False for ETF columns, True for everything else."""
    stock_cols = [c for c in df.columns if c.upper() not in etf_tickers]
    mask = pd.DataFrame(False, index=df.index, columns=df.columns)
    mask[stock_cols] = True
    return mask


def rs_percentile_ranks(
    returns: dict[int, pd.DataFrame],
) -> dict[str, pd.DataFrame]:
    """Compute RS percentile ranks as float DataFrames (0.0-1.0).

    Cache these once; apply threshold at query time via rank_relative_strength(..., precomputed=...).
    Returns dict with keys: pctrank_1m, pctrank_3m, pctrank_6m.
    """
    r1 = returns.get(21, returns[min(returns.keys())])
    r3 = returns.get(63, r1)
    r6 = returns.get(126, r3)
    return {
        "pctrank_1m": r1.rank(axis=1, pct=True, na_option="keep"),
        "pctrank_3m": r3.rank(axis=1, pct=True, na_option="keep"),
        "pctrank_6m": r6.rank(axis=1, pct=True, na_option="keep"),
    }


def rank_relative_strength(
    returns: dict[int, pd.DataFrame],
    top_pct: float = 0.10,
    precomputed: dict | None = None,
) -> pd.DataFrame:
    """Boolean mask: True for tickers in the top N% RS across ALL timeframes.

    Qullamaggie screens for stocks that appear in the top tier simultaneously
    across 1-month, 3-month, AND 6-month raw return rankings — not a weighted
    composite. A stock must be a leader across all three windows to qualify.

    Default top_pct=0.10 (top 10%) is practical for backtesting. Qullamaggie
    uses 1-2% manually but that yields very few backtest signals when applied
    mechanically across 8k tickers.

    Pass precomputed=rs_percentile_ranks(...) to skip the expensive rank() calls.
    """
    if precomputed is not None:
        pct1 = precomputed["pctrank_1m"]
        pct3 = precomputed["pctrank_3m"]
        pct6 = precomputed["pctrank_6m"]
    else:
        ranks = rs_percentile_ranks(returns)
        pct1 = ranks["pctrank_1m"]
        pct3 = ranks["pctrank_3m"]
        pct6 = ranks["pctrank_6m"]

    threshold = 1.0 - top_pct
    # Must be a leader across all three windows simultaneously
    return (pct1 >= threshold) & (pct3 >= threshold) & (pct6 >= threshold)


def mask_near_52w_high(dist_52w: pd.DataFrame, max_dist: float = 0.25) -> pd.DataFrame:
    """Boolean mask: True where ticker is within max_dist (25%) of 52-week high."""
    return dist_52w <= max_dist


def mask_prior_move(ret_63: pd.DataFrame, min_ret: float = 0.30) -> pd.DataFrame:
    """True where 3-month return >= min_ret. Ensures stock already has momentum."""
    return ret_63 >= min_ret


def mask_ma_proximity(close: pd.DataFrame, sma: pd.DataFrame, max_ext: float = 0.10) -> pd.DataFrame:
    """True where close is not more than max_ext above the moving average.

    Filters out stocks already extended above their SMA; keeps consolidations near it.
    """
    safe_sma = sma.where(sma > 0)
    ext = (close - safe_sma) / safe_sma
    return ext <= max_ext


def get_breakout_universe(
    liquid: pd.DataFrame,
    rs_top: pd.DataFrame,
    near_high: pd.DataFrame,
    prior_move: pd.DataFrame | None = None,
    ma_proximity: pd.DataFrame | None = None,
) -> pd.DataFrame:
    """Breakout candidates: liquid AND top RS AND near 52w high.

    Optional quality filters applied when provided:
    - prior_move: 3M return >= threshold (stock already has momentum)
    - ma_proximity: close <= threshold above 10-day SMA (not over-extended)

    Returns wide boolean DataFrame — True cells are valid breakout candidates
    on that date.
    """
    mask = liquid & rs_top & near_high
    if prior_move is not None:
        mask = mask & prior_move
    if ma_proximity is not None:
        mask = mask & ma_proximity
    return mask


def get_ep_universe(
    open_: pd.DataFrame,
    close: pd.DataFrame,
    volume: pd.DataFrame,
    vol_sma: pd.DataFrame,
    liquid: pd.DataFrame,
    min_gap_pct: float = 0.10,
    min_vol_ratio: float = 2.0,
) -> pd.DataFrame:
    """EP candidates: gap up >= min_gap_pct AND volume >= min_vol_ratio × avg, AND liquid.

    Note: this marks the GAP DAY itself. Entry is the following day.
    """
    prev_close = close.shift(1)
    gap = (open_ / prev_close - 1) >= min_gap_pct
    vol_spike = volume >= (min_vol_ratio * vol_sma)
    return liquid & gap & vol_spike


def get_parabolic_universe(
    close: pd.DataFrame,
    open_: pd.DataFrame,
    pct_10d: pd.DataFrame,
    consec_green: pd.DataFrame,
    liquid: pd.DataFrame,
    large_cap_price: float = 50.0,
    large_cap_run: float = 0.50,
    small_cap_run: float = 3.00,
    min_green_days: int = 3,
) -> pd.DataFrame:
    """Parabolic short candidates.

    Uses price as a proxy for market cap:
    - price > large_cap_price: needs 50%+ run in 10d
    - price <= large_cap_price: needs 300%+ run in 10d
    Plus 3+ consecutive green days on prior day.
    """
    is_large = close > large_cap_price
    run_ok = (is_large & (pct_10d >= large_cap_run)) | (~is_large & (pct_10d >= small_cap_run))
    green_ok = consec_green.shift(1) >= min_green_days
    return liquid & run_ok & green_ok
