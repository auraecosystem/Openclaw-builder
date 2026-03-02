"""
Vectorized technical indicators operating on wide DataFrames (DatetimeIndex × tickers).

All functions accept and return DataFrames of the same shape — no loops over tickers.
Use float32 inputs for memory efficiency (~3GB for full 11k-ticker universe).
"""

import numpy as np
import pandas as pd


def atr(high: pd.DataFrame, low: pd.DataFrame, close: pd.DataFrame, period: int = 14) -> pd.DataFrame:
    """Average True Range across all tickers simultaneously.

    TR = max(high-low, |high-prev_close|, |low-prev_close|)
    ATR = EWM(TR, span=period)
    """
    prev_close = close.shift(1)
    tr = pd.concat([
        high - low,
        (high - prev_close).abs(),
        (low - prev_close).abs(),
    ]).groupby(level=0).max()
    # Wilder's smoothing: alpha = 1/n (not standard EMA alpha = 2/(n+1))
    return tr.ewm(alpha=1.0 / period, min_periods=period, adjust=False).mean()


def sma(df: pd.DataFrame, period: int) -> pd.DataFrame:
    """Simple moving average across all tickers."""
    return df.rolling(period, min_periods=period).mean()


def ema(df: pd.DataFrame, period: int) -> pd.DataFrame:
    """Exponential moving average across all tickers."""
    return df.ewm(span=period, min_periods=period, adjust=False).mean()


def volume_sma(volume: pd.DataFrame, period: int = 20) -> pd.DataFrame:
    """20-day simple moving average of volume across all tickers."""
    return sma(volume, period)


def rolling_return(close: pd.DataFrame, periods: list[int] = None) -> dict[int, pd.DataFrame]:
    """Percentage return over N trading days for each period.

    Returns dict mapping period → wide DataFrame of % returns.
    e.g., periods=[21, 63, 126] ≈ 1M, 3M, 6M returns.
    """
    if periods is None:
        periods = [21, 63, 126]
    return {p: close.pct_change(periods=p) for p in periods}


def high_52w(high: pd.DataFrame) -> pd.DataFrame:
    """Rolling 252-day max of high prices across all tickers."""
    return high.rolling(252, min_periods=63).max()


def distance_from_52w_high(close: pd.DataFrame, high: pd.DataFrame) -> pd.DataFrame:
    """Fraction below 52-week high. 0 = at high, 0.25 = 25% below.

    Returns positive values representing distance below the high.
    """
    h52 = high_52w(high)
    return (h52 - close) / h52


def pct_change_n_days(close: pd.DataFrame, n: int) -> pd.DataFrame:
    """Percentage change over the last N trading days across all tickers."""
    return close.pct_change(periods=n)


def consecutive_green_days(open_: pd.DataFrame, close: pd.DataFrame) -> pd.DataFrame:
    """Rolling count of consecutive days where close > open (green candle).

    Resets to 0 on any red day. Implemented via numba for performance;
    falls back to a pure-pandas approach if numba is unavailable.
    """
    is_green = (close > open_).astype(np.int8)

    try:
        from numba import njit, prange  # noqa: F401
        return _consecutive_green_numba(is_green)
    except ImportError:
        return _consecutive_green_pandas(is_green)


def _consecutive_green_pandas(is_green: pd.DataFrame) -> pd.DataFrame:
    """Pure-pandas streak counter (slower but no numba dependency)."""
    # Group consecutive runs and cumsum within each green run
    arr = is_green.values
    result = np.zeros_like(arr, dtype=np.int32)
    for col in range(arr.shape[1]):
        streak = 0
        for row in range(arr.shape[0]):
            if arr[row, col] == 1:
                streak += 1
            else:
                streak = 0
            result[row, col] = streak
    return pd.DataFrame(result, index=is_green.index, columns=is_green.columns)


def _consecutive_green_numba(is_green: pd.DataFrame) -> pd.DataFrame:
    """Numba-accelerated streak counter with column-level parallelism."""
    from numba import njit, prange

    @njit(parallel=True)
    def _streak(arr):
        n_rows, n_cols = arr.shape
        result = np.zeros((n_rows, n_cols), dtype=np.int32)
        for col in prange(n_cols):
            streak = 0
            for row in range(n_rows):
                if arr[row, col] == 1:
                    streak += 1
                else:
                    streak = 0
                result[row, col] = streak
        return result

    result = _streak(is_green.values)
    return pd.DataFrame(result, index=is_green.index, columns=is_green.columns)
