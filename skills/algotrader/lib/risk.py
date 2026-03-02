"""
Position sizing and stop management.

size_from_risk(): ATR-based position sizing — risk a fixed % of equity per trade.
compute_stop_exit(): stateful stop tracking across time (numba-accelerated).
compute_partial_exit(): exit after N days only if profitable (for Qullamaggie partial profit-taking).
compute_breakeven_stop_exit(): stop upgrades to breakeven after N days if profitable.
"""

import numpy as np
import pandas as pd


def size_adjusted_slippage(
    sizes: pd.DataFrame,
    vol_sma_20: pd.DataFrame,
    entries: pd.DataFrame,
    base_slippage: float = 0.001,
    k: float = 0.1,
) -> pd.DataFrame:
    """Per-bar slippage DataFrame for vectorbt's slippage= param.

    Entry bars: base + k * sqrt(shares / ADV). All other bars: base_slippage.
    Capped at 5% to avoid outliers on ultra-illiquid fills.
    """
    adv = vol_sma_20.reindex(index=sizes.index, columns=sizes.columns)
    impact = k * np.sqrt(sizes / adv.where(adv > 0))
    slippage = impact.where(entries, other=base_slippage).fillna(base_slippage)
    return slippage.clip(lower=base_slippage, upper=0.05)


def size_from_risk(
    close: pd.DataFrame,
    stop_prices: pd.DataFrame,
    entries: pd.DataFrame,
    equity: float,
    risk_pct: float = 0.005,
    max_pos_pct: float = 0.20,
) -> pd.DataFrame:
    """Share count at each entry bar.

    shares = (equity × risk_pct) / |close - stop_price|
    Capped at max_pos_pct × equity / close (no single position > 20% of portfolio).
    Non-entry cells are NaN (vectorbt ignores NaN sizes on non-entry bars).
    """
    risk_dollars = equity * risk_pct
    price_risk = (close - stop_prices).abs().replace(0, np.nan)
    raw_shares = risk_dollars / price_risk

    max_shares = (equity * max_pos_pct) / close.replace(0, np.nan)
    sizes = raw_shares.clip(upper=max_shares)

    # Only populate on entry days; NaN elsewhere
    return sizes.where(entries)


def compute_stop_exit(
    close: pd.DataFrame,
    entries: pd.DataFrame,
    stop_prices: pd.DataFrame,
) -> pd.DataFrame:
    """Boolean mask: True when price breaches the active stop for a position.

    For each ticker, tracks the stop price set at the most recent entry and
    marks True on any subsequent bar where close falls at or below that stop.
    Uses numba for column-parallel execution; falls back to pure Python.
    """
    try:
        return _stop_exit_numba(close, entries, stop_prices)
    except ImportError:
        return _stop_exit_python(close, entries, stop_prices)


def _stop_exit_python(
    close: pd.DataFrame,
    entries: pd.DataFrame,
    stop_prices: pd.DataFrame,
) -> pd.DataFrame:
    """Pure Python fallback for stop exit computation."""
    close_arr = close.values.astype(np.float32)
    entry_arr = entries.values.astype(bool)
    stop_arr = stop_prices.values.astype(np.float32)

    n_rows, n_cols = close_arr.shape
    result = np.zeros((n_rows, n_cols), dtype=bool)

    for col in range(n_cols):
        active_stop = np.nan
        for row in range(n_rows):
            if entry_arr[row, col]:
                active_stop = stop_arr[row, col]
            elif not np.isnan(active_stop):
                if close_arr[row, col] <= active_stop:
                    result[row, col] = True
                    active_stop = np.nan  # stop triggered, position closed

    return pd.DataFrame(result, index=close.index, columns=close.columns)


def _stop_exit_numba(
    close: pd.DataFrame,
    entries: pd.DataFrame,
    stop_prices: pd.DataFrame,
) -> pd.DataFrame:
    """Numba-accelerated stop exit with column-level parallelism."""
    from numba import njit, prange

    @njit(parallel=True)
    def _scan(close_arr, entry_arr, stop_arr):
        n_rows, n_cols = close_arr.shape
        result = np.zeros((n_rows, n_cols), dtype=np.bool_)
        for col in prange(n_cols):
            active_stop = np.nan
            for row in range(n_rows):
                if entry_arr[row, col]:
                    active_stop = stop_arr[row, col]
                elif not np.isnan(active_stop):
                    if close_arr[row, col] <= active_stop:
                        result[row, col] = True
                        active_stop = np.nan
        return result

    result = _scan(
        close.values.astype(np.float32),
        entries.values.astype(np.bool_),
        stop_prices.values.astype(np.float32),
    )
    return pd.DataFrame(result, index=close.index, columns=close.columns)


def compute_partial_exit(
    close: pd.DataFrame,
    entries: pd.DataFrame,
    n_days: int = 5,
) -> pd.DataFrame:
    """Exit signal N trading days after entry, ONLY if position is profitable.

    Implements Qullamaggie's rule: "sell 1/3 to 1/2 after 3-5 days if profitable."
    If not profitable at day N, no exit fires — position continues with its stop/trail.
    """
    try:
        return _partial_exit_numba(close, entries, n_days)
    except ImportError:
        return _partial_exit_python(close, entries, n_days)


def _partial_exit_python(
    close: pd.DataFrame,
    entries: pd.DataFrame,
    n_days: int,
) -> pd.DataFrame:
    close_arr = close.values.astype(np.float32)
    entry_arr = entries.values.astype(bool)
    n_rows, n_cols = close_arr.shape
    result = np.zeros((n_rows, n_cols), dtype=bool)

    for col in range(n_cols):
        entry_price = np.nan
        bars_since_entry = 0
        in_position = False

        for row in range(n_rows):
            if entry_arr[row, col]:
                entry_price = close_arr[row, col]
                bars_since_entry = 0
                in_position = True
            elif in_position:
                bars_since_entry += 1
                if bars_since_entry >= n_days and close_arr[row, col] > entry_price:
                    result[row, col] = True
                    in_position = False

    return pd.DataFrame(result, index=close.index, columns=close.columns)


def _partial_exit_numba(
    close: pd.DataFrame,
    entries: pd.DataFrame,
    n_days: int,
) -> pd.DataFrame:
    from numba import njit, prange

    @njit(parallel=True)
    def _scan(close_arr, entry_arr, n_days):
        n_rows, n_cols = close_arr.shape
        result = np.zeros((n_rows, n_cols), dtype=np.bool_)
        for col in prange(n_cols):
            entry_price = np.nan
            bars_since = 0
            in_pos = False
            for row in range(n_rows):
                if entry_arr[row, col]:
                    entry_price = close_arr[row, col]
                    bars_since = 0
                    in_pos = True
                elif in_pos:
                    bars_since += 1
                    if bars_since >= n_days and close_arr[row, col] > entry_price:
                        result[row, col] = True
                        in_pos = False
        return result

    result = _scan(
        close.values.astype(np.float32),
        entries.values.astype(np.bool_),
        n_days,
    )
    return pd.DataFrame(result, index=close.index, columns=close.columns)


def compute_breakeven_stop_exit(
    close: pd.DataFrame,
    entries: pd.DataFrame,
    stop_prices: pd.DataFrame,
    n_days: int = 5,
) -> pd.DataFrame:
    """Stop exit that upgrades to breakeven after N days if profitable.

    Implements Qullamaggie's rule: "move stop to breakeven on remainder" after
    taking partial profits. Before day N: original stop. After day N: if close >
    entry price, stop upgrades to entry price (breakeven). Stop only moves up.
    """
    try:
        return _breakeven_stop_numba(close, entries, stop_prices, n_days)
    except ImportError:
        return _breakeven_stop_python(close, entries, stop_prices, n_days)


def _breakeven_stop_python(
    close: pd.DataFrame,
    entries: pd.DataFrame,
    stop_prices: pd.DataFrame,
    n_days: int,
) -> pd.DataFrame:
    close_arr = close.values.astype(np.float32)
    entry_arr = entries.values.astype(bool)
    stop_arr = stop_prices.values.astype(np.float32)
    n_rows, n_cols = close_arr.shape
    result = np.zeros((n_rows, n_cols), dtype=bool)

    for col in range(n_cols):
        active_stop = np.nan
        entry_price = np.nan
        bars_since = 0

        for row in range(n_rows):
            if entry_arr[row, col]:
                active_stop = stop_arr[row, col]
                entry_price = close_arr[row, col]
                bars_since = 0
            elif not np.isnan(active_stop):
                bars_since += 1
                # After n_days: upgrade stop to breakeven if profitable
                if bars_since >= n_days and close_arr[row, col] > entry_price:
                    active_stop = max(active_stop, entry_price)
                if close_arr[row, col] <= active_stop:
                    result[row, col] = True
                    active_stop = np.nan
                    entry_price = np.nan

    return pd.DataFrame(result, index=close.index, columns=close.columns)


def _breakeven_stop_numba(
    close: pd.DataFrame,
    entries: pd.DataFrame,
    stop_prices: pd.DataFrame,
    n_days: int,
) -> pd.DataFrame:
    from numba import njit, prange

    @njit(parallel=True)
    def _scan(close_arr, entry_arr, stop_arr, n_days):
        n_rows, n_cols = close_arr.shape
        result = np.zeros((n_rows, n_cols), dtype=np.bool_)
        for col in prange(n_cols):
            active_stop = np.nan
            entry_price = np.nan
            bars_since = 0
            for row in range(n_rows):
                if entry_arr[row, col]:
                    active_stop = stop_arr[row, col]
                    entry_price = close_arr[row, col]
                    bars_since = 0
                elif not np.isnan(active_stop):
                    bars_since += 1
                    if bars_since >= n_days and close_arr[row, col] > entry_price:
                        active_stop = max(active_stop, entry_price)
                    if close_arr[row, col] <= active_stop:
                        result[row, col] = True
                        active_stop = np.nan
                        entry_price = np.nan
        return result

    result = _scan(
        close.values.astype(np.float32),
        entries.values.astype(np.bool_),
        stop_prices.values.astype(np.float32),
        n_days,
    )
    return pd.DataFrame(result, index=close.index, columns=close.columns)
