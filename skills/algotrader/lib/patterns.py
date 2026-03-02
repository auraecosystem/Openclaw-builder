"""
Pattern detection: VCP, flags, and consolidation structures.

All functions operate on wide DataFrames (DatetimeIndex × tickers) and return
wide float DataFrames of the same shape. Swing-point detection is vectorized;
VCP/flag contraction counting uses numba @njit(parallel=True) for per-ticker
stateful iteration.

Pre-computation for fast threshold sweeps
-----------------------------------------
Use vcp_intermediates() and flag_intermediates() to compute the expensive
detection once (and cache as parquet). The backtest loads these and applies
thresholds at runtime — changing thresholds costs nothing.
"""

import numpy as np
import pandas as pd


# ---------------------------------------------------------------------------
# Consolidation primitives (unchanged — used for breakout level)
# ---------------------------------------------------------------------------

def consolidation_range(
    high: pd.DataFrame,
    low: pd.DataFrame,
    lookback: int = 40,
) -> pd.DataFrame:
    """Range as fraction of high over the lookback window.

    (rolling_max_high - rolling_min_low) / rolling_max_high
    Lower values = tighter consolidation.
    """
    roll_high = high.rolling(lookback, min_periods=lookback // 2).max()
    roll_low = low.rolling(lookback, min_periods=lookback // 2).min()
    return (roll_high - roll_low) / roll_high


def consolidation_high(high: pd.DataFrame, lookback: int = 40) -> pd.DataFrame:
    """Rolling max of PRIOR highs — the breakout level.

    Uses shift(1) so today's high is excluded. A breakout occurs when today's
    close exceeds the highest high of the prior N days.
    """
    return high.shift(1).rolling(lookback, min_periods=lookback // 2).max()


# ---------------------------------------------------------------------------
# Composite pattern detection (VCP | Flag)
# ---------------------------------------------------------------------------

def is_any_pattern(
    high: pd.DataFrame,
    low: pd.DataFrame,
    close: pd.DataFrame,
    volume: pd.DataFrame,
    vol_sma_20: pd.DataFrame,
    max_range_pct: float = 0.15,
    vcp_precomputed: dict[str, pd.DataFrame] | None = None,
    flag_precomputed: dict[str, pd.DataFrame] | None = None,
    # VCP thresholds
    min_contractions: int = 2,
    max_tightening: float = 1.0,
    # Flag thresholds
    flag_min_pole: float = 0.20,
    flag_max_retrace: float = 0.50,
    flag_max_vol_ratio: float = 1.0,
) -> pd.DataFrame:
    """True where either VCP or bull flag pattern is detected.

    Uses pre-computed intermediates when available (fast threshold sweeps).
    Falls back to computing from scratch if not provided.

    Returns a boolean DataFrame (same shape as inputs).
    """
    # VCP detection
    vcp = vcp_precomputed or vcp_intermediates(high, low, close, volume)
    is_vcp = (
        (vcp["num_contractions"] >= min_contractions)
        & (vcp["tightening_ratio"] < max_tightening)
    )

    # Flag detection
    flag = flag_precomputed or flag_intermediates(high, low, close, volume)
    is_flag = (
        (flag["pole_pct"] >= flag_min_pole)
        & (flag["retrace_pct"] <= flag_max_retrace)
        & (flag["vol_ratio"] < flag_max_vol_ratio)
    )

    # Consolidation range filter (applies to both patterns)
    consol = consolidation_range(high, low)
    in_range = consol <= max_range_pct

    return (is_vcp | is_flag) & in_range


# ---------------------------------------------------------------------------
# Swing point detection
# ---------------------------------------------------------------------------

def detect_swing_points(
    high: pd.DataFrame,
    low: pd.DataFrame,
    order: int = 5,
) -> tuple[pd.DataFrame, pd.DataFrame]:
    """Detect swing highs and swing lows per ticker.

    A swing high at bar i: high[i] is the max of high[i-order : i+order+1].
    A swing low at bar i: low[i] is the min of low[i-order : i+order+1].

    Returns (swing_highs, swing_lows) as float DataFrames.
    Non-swing bars are NaN. Swing bars contain the price value.
    Lagged by `order` bars (needs future confirmation).
    """
    window = 2 * order + 1

    # Rolling max of high / rolling min of low centered on each bar
    roll_max = high.rolling(window, center=True, min_periods=window).max()
    roll_min = low.rolling(window, center=True, min_periods=window).min()

    # Swing high: bar's high equals the rolling max
    swing_highs = high.where(high == roll_max)
    # Swing low: bar's low equals the rolling min
    swing_lows = low.where(low == roll_min)

    return swing_highs, swing_lows


# ---------------------------------------------------------------------------
# VCP: swing-point contraction counting (numba-accelerated)
# ---------------------------------------------------------------------------

def _vcp_kernel_numpy(
    high_arr: np.ndarray,
    low_arr: np.ndarray,
    volume_arr: np.ndarray,
    sh_arr: np.ndarray,
    sl_arr: np.ndarray,
    lookback: int,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    """Pure numpy fallback for VCP contraction counting. Slow but correct."""
    n_rows, n_cols = high_arr.shape
    num_contr = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
    last_contr = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
    tight_ratio = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
    vol_trend = np.full((n_rows, n_cols), np.nan, dtype=np.float32)

    for col in range(n_cols):
        for row in range(lookback, n_rows):
            _vcp_single(
                high_arr, low_arr, volume_arr, sh_arr, sl_arr,
                row, col, lookback,
                num_contr, last_contr, tight_ratio, vol_trend,
            )
    return num_contr, last_contr, tight_ratio, vol_trend


def _vcp_single(
    high_arr, low_arr, volume_arr, sh_arr, sl_arr,
    row, col, lookback,
    num_contr, last_contr, tight_ratio, vol_trend,
):
    """Process one (row, col) for VCP detection."""
    start = max(0, row - lookback)

    # Collect swing highs and swing lows in the lookback window
    swing_highs = []  # (bar_idx, price)
    swing_lows = []
    for r in range(start, row + 1):
        sh = sh_arr[r, col]
        sl = sl_arr[r, col]
        if not np.isnan(sh):
            swing_highs.append((r, sh))
        if not np.isnan(sl):
            swing_lows.append((r, sl))

    if len(swing_highs) < 1 or len(swing_lows) < 1:
        return

    # Build contractions: pair each swing high with the next swing low after it
    contractions = []  # (sh_price, sl_price, sh_row, sl_row)
    si = 0
    for sh_row, sh_price in swing_highs:
        # Find the next swing low AFTER this swing high
        while si < len(swing_lows) and swing_lows[si][0] <= sh_row:
            si += 1
        if si < len(swing_lows):
            sl_row, sl_price = swing_lows[si]
            if sh_price > 0:
                contractions.append((sh_price, sl_price, sh_row, sl_row))

    if len(contractions) < 1:
        return

    # Count progressive contractions (each range < previous)
    ranges = [(c[0] - c[1]) / c[0] for c in contractions]
    progressive = 1
    for i in range(1, len(ranges)):
        if ranges[i] < ranges[i - 1]:
            progressive += 1
        else:
            break

    # Only count if we have at least the first contraction
    num_contr[row, col] = progressive
    last_contr[row, col] = ranges[-1] if ranges else np.nan

    if len(ranges) >= 2 and ranges[0] > 0:
        tight_ratio[row, col] = ranges[-1] / ranges[0]
    elif len(ranges) == 1:
        tight_ratio[row, col] = 1.0

    # Volume trend: avg volume in last contraction vs first contraction
    first_c = contractions[0]
    last_c = contractions[-1]
    vol_first = _avg_volume_range(volume_arr, col, first_c[2], first_c[3])
    vol_last = _avg_volume_range(volume_arr, col, last_c[2], last_c[3])
    if vol_first > 0:
        vol_trend[row, col] = vol_last / vol_first


def _avg_volume_range(volume_arr, col, start_row, end_row):
    """Average volume between two rows (inclusive)."""
    if end_row < start_row:
        return 0.0
    total = 0.0
    count = 0
    for r in range(start_row, end_row + 1):
        v = volume_arr[r, col]
        if not np.isnan(v):
            total += v
            count += 1
    return total / count if count > 0 else 0.0


def _make_vcp_numba_kernel():
    """Build and return the numba-jitted VCP kernel."""
    from numba import njit, prange

    @njit(parallel=True)
    def _vcp_numba(high_arr, low_arr, volume_arr, sh_arr, sl_arr, lookback):
        n_rows, n_cols = high_arr.shape
        num_contr = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
        last_contr = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
        tight_ratio = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
        vol_trend_out = np.full((n_rows, n_cols), np.nan, dtype=np.float32)

        for col in prange(n_cols):
            for row in range(lookback, n_rows):
                start = row - lookback
                if start < 0:
                    start = 0

                # Collect swing points in lookback window
                sh_prices = np.empty(lookback + 1, dtype=np.float32)
                sh_rows = np.empty(lookback + 1, dtype=np.int64)
                n_sh = 0
                sl_prices = np.empty(lookback + 1, dtype=np.float32)
                sl_rows = np.empty(lookback + 1, dtype=np.int64)
                n_sl = 0

                for r in range(start, row + 1):
                    sh = sh_arr[r, col]
                    sl = sl_arr[r, col]
                    if not np.isnan(sh):
                        sh_prices[n_sh] = sh
                        sh_rows[n_sh] = r
                        n_sh += 1
                    if not np.isnan(sl):
                        sl_prices[n_sl] = sl
                        sl_rows[n_sl] = r
                        n_sl += 1

                if n_sh < 1 or n_sl < 1:
                    continue

                # Build contractions: pair swing high → next swing low
                c_sh = np.empty(n_sh, dtype=np.float32)
                c_sl = np.empty(n_sh, dtype=np.float32)
                c_sh_row = np.empty(n_sh, dtype=np.int64)
                c_sl_row = np.empty(n_sh, dtype=np.int64)
                n_c = 0
                si = 0

                for i in range(n_sh):
                    sh_r = sh_rows[i]
                    # Advance to next swing low after this swing high
                    while si < n_sl and sl_rows[si] <= sh_r:
                        si += 1
                    if si < n_sl and sh_prices[i] > 0:
                        c_sh[n_c] = sh_prices[i]
                        c_sl[n_c] = sl_prices[si]
                        c_sh_row[n_c] = sh_r
                        c_sl_row[n_c] = sl_rows[si]
                        n_c += 1

                if n_c < 1:
                    continue

                # Compute contraction ranges as % of swing high
                ranges = np.empty(n_c, dtype=np.float32)
                for i in range(n_c):
                    ranges[i] = (c_sh[i] - c_sl[i]) / c_sh[i]

                # Count progressive contractions from the start
                progressive = 1
                for i in range(1, n_c):
                    if ranges[i] < ranges[i - 1]:
                        progressive += 1
                    else:
                        break

                num_contr[row, col] = progressive
                last_contr[row, col] = ranges[n_c - 1]

                if n_c >= 2 and ranges[0] > 0:
                    tight_ratio[row, col] = ranges[n_c - 1] / ranges[0]
                elif n_c == 1:
                    tight_ratio[row, col] = 1.0

                # Volume trend: last contraction vs first contraction
                # Avg volume in contraction = mean of volume[sh_row..sl_row]
                vol_first = 0.0
                cnt_first = 0
                for r in range(c_sh_row[0], c_sl_row[0] + 1):
                    v = volume_arr[r, col]
                    if not np.isnan(v):
                        vol_first += v
                        cnt_first += 1
                if cnt_first > 0:
                    vol_first /= cnt_first

                vol_last = 0.0
                cnt_last = 0
                last_idx = n_c - 1
                for r in range(c_sh_row[last_idx], c_sl_row[last_idx] + 1):
                    v = volume_arr[r, col]
                    if not np.isnan(v):
                        vol_last += v
                        cnt_last += 1
                if cnt_last > 0:
                    vol_last /= cnt_last

                if vol_first > 0:
                    vol_trend_out[row, col] = vol_last / vol_first

        return num_contr, last_contr, tight_ratio, vol_trend_out

    return _vcp_numba


def vcp_intermediates(
    high: pd.DataFrame,
    low: pd.DataFrame,
    close: pd.DataFrame,
    volume: pd.DataFrame,
    swing_order: int = 5,
    lookback: int = 60,
) -> dict[str, pd.DataFrame]:
    """Detect VCP contractions via swing points.

    For each bar, looks back up to `lookback` days and:
    1. Finds swing highs and swing lows
    2. Pairs them into contractions (swing_high → next swing_low)
    3. Counts progressive contractions (each smaller than previous)
    4. Measures last contraction size, tightening ratio, volume trend

    Returns dict with keys:
      - num_contractions: count of progressive contractions (float, 0-6)
      - last_contraction_pct: range of final contraction as % of pivot high
      - tightening_ratio: last_contraction / first_contraction (< 1 = good)
      - vol_trend: avg volume in last contraction / first (< 1 = drying up)
    """
    swing_highs, swing_lows = detect_swing_points(high, low, order=swing_order)

    h = high.values.astype(np.float32)
    l = low.values.astype(np.float32)
    v = volume.values.astype(np.float32)
    sh = swing_highs.values.astype(np.float32)
    sl = swing_lows.values.astype(np.float32)

    try:
        kernel = _make_vcp_numba_kernel()
        num_c, last_c, tight, vol_t = kernel(h, l, v, sh, sl, lookback)
    except ImportError:
        num_c, last_c, tight, vol_t = _vcp_kernel_numpy(h, l, v, sh, sl, lookback)

    idx, cols = high.index, high.columns
    return {
        "num_contractions": pd.DataFrame(num_c, index=idx, columns=cols),
        "last_contraction_pct": pd.DataFrame(last_c, index=idx, columns=cols),
        "tightening_ratio": pd.DataFrame(tight, index=idx, columns=cols),
        "vol_trend": pd.DataFrame(vol_t, index=idx, columns=cols),
    }


# ---------------------------------------------------------------------------
# Flag: impulse (pole) + consolidation detection (numba-accelerated)
# ---------------------------------------------------------------------------

def _flag_kernel_numpy(
    high_arr: np.ndarray,
    low_arr: np.ndarray,
    close_arr: np.ndarray,
    volume_arr: np.ndarray,
    sh_arr: np.ndarray,
    sl_arr: np.ndarray,
    pole_max_days: int,
    flag_max_days: int,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    """Pure numpy fallback for flag detection. Slow but correct."""
    n_rows, n_cols = close_arr.shape
    pole_pct = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
    retrace_pct = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
    flag_days_out = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
    vol_ratio = np.full((n_rows, n_cols), np.nan, dtype=np.float32)

    max_look = pole_max_days + flag_max_days

    for col in range(n_cols):
        for row in range(max_look, n_rows):
            _flag_single(
                high_arr, low_arr, close_arr, volume_arr, sh_arr, sl_arr,
                row, col, pole_max_days, flag_max_days,
                pole_pct, retrace_pct, flag_days_out, vol_ratio,
            )
    return pole_pct, retrace_pct, flag_days_out, vol_ratio


def _flag_single(
    high_arr, low_arr, close_arr, volume_arr, sh_arr, sl_arr,
    row, col, pole_max_days, flag_max_days,
    pole_pct, retrace_pct, flag_days_out, vol_ratio,
):
    """Process one (row, col) for flag detection."""
    # Look for a swing high (pole top) within flag_max_days before current bar
    peak_row = -1
    peak_price = 0.0
    for r in range(row - 1, max(row - flag_max_days - 1, -1), -1):
        sh = sh_arr[r, col]
        if not np.isnan(sh) and sh > peak_price:
            peak_row = r
            peak_price = sh

    if peak_row < 0 or peak_price <= 0:
        return

    # Look for the pole start (swing low before the peak, within pole_max_days)
    pole_start_row = -1
    pole_low = peak_price
    search_start = max(0, peak_row - pole_max_days)
    for r in range(peak_row - 1, search_start - 1, -1):
        sl = sl_arr[r, col]
        if not np.isnan(sl) and sl < pole_low:
            pole_low = sl
            pole_start_row = r

    if pole_start_row < 0 or pole_low <= 0:
        return

    # Pole size
    pole_gain = (peak_price - pole_low) / pole_low
    if pole_gain < 0.15:  # minimum 15% pole to consider (flag threshold is 20% in engine)
        return

    pole_pct[row, col] = pole_gain

    # Flag metrics: from peak to current bar
    flag_len = row - peak_row
    flag_days_out[row, col] = flag_len

    # Retrace: how much of the pole move has been given back
    current_close = close_arr[row, col]
    if np.isnan(current_close):
        return
    move_size = peak_price - pole_low
    retrace = (peak_price - current_close) / move_size if move_size > 0 else np.nan
    retrace_pct[row, col] = retrace

    # Volume: avg during flag / avg during pole
    vol_flag = _avg_vol(volume_arr, col, peak_row, row)
    vol_pole = _avg_vol(volume_arr, col, pole_start_row, peak_row)
    if vol_pole > 0:
        vol_ratio[row, col] = vol_flag / vol_pole


def _avg_vol(volume_arr, col, start, end):
    """Average volume from start to end (inclusive)."""
    total = 0.0
    count = 0
    for r in range(start, end + 1):
        v = volume_arr[r, col]
        if not np.isnan(v):
            total += v
            count += 1
    return total / count if count > 0 else 0.0


def _make_flag_numba_kernel():
    """Build and return the numba-jitted flag kernel."""
    from numba import njit, prange

    @njit(parallel=True)
    def _flag_numba(high_arr, low_arr, close_arr, volume_arr, sh_arr, sl_arr,
                    pole_max_days, flag_max_days):
        n_rows, n_cols = close_arr.shape
        pole_pct = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
        retrace_pct = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
        flag_days_out = np.full((n_rows, n_cols), np.nan, dtype=np.float32)
        vol_ratio_out = np.full((n_rows, n_cols), np.nan, dtype=np.float32)

        max_look = pole_max_days + flag_max_days

        for col in prange(n_cols):
            for row in range(max_look, n_rows):
                # Find swing high (pole top) within flag_max_days before bar
                peak_row = -1
                peak_price = np.float32(0.0)
                limit = row - flag_max_days - 1
                if limit < -1:
                    limit = -1
                for r in range(row - 1, limit, -1):
                    sh = sh_arr[r, col]
                    if not np.isnan(sh) and sh > peak_price:
                        peak_row = r
                        peak_price = sh

                if peak_row < 0 or peak_price <= 0:
                    continue

                # Find pole start (swing low before peak, within pole_max_days)
                pole_start_row = -1
                pole_low = peak_price
                search_start = peak_row - pole_max_days
                if search_start < 0:
                    search_start = 0
                for r in range(peak_row - 1, search_start - 1, -1):
                    sl = sl_arr[r, col]
                    if not np.isnan(sl) and sl < pole_low:
                        pole_low = sl
                        pole_start_row = r

                if pole_start_row < 0 or pole_low <= 0:
                    continue

                pole_gain = (peak_price - pole_low) / pole_low
                if pole_gain < 0.15:
                    continue

                pole_pct[row, col] = pole_gain

                flag_len = row - peak_row
                flag_days_out[row, col] = flag_len

                current_close = close_arr[row, col]
                if np.isnan(current_close):
                    continue
                move_size = peak_price - pole_low
                if move_size > 0:
                    retrace_pct[row, col] = (peak_price - current_close) / move_size

                # Volume ratio: flag period vs pole period
                vol_flag = np.float32(0.0)
                cnt_flag = 0
                for r in range(peak_row, row + 1):
                    v = volume_arr[r, col]
                    if not np.isnan(v):
                        vol_flag += v
                        cnt_flag += 1
                if cnt_flag > 0:
                    vol_flag /= cnt_flag

                vol_pole = np.float32(0.0)
                cnt_pole = 0
                for r in range(pole_start_row, peak_row + 1):
                    v = volume_arr[r, col]
                    if not np.isnan(v):
                        vol_pole += v
                        cnt_pole += 1
                if cnt_pole > 0:
                    vol_pole /= cnt_pole

                if vol_pole > 0:
                    vol_ratio_out[row, col] = vol_flag / vol_pole

        return pole_pct, retrace_pct, flag_days_out, vol_ratio_out

    return _flag_numba


def flag_intermediates(
    high: pd.DataFrame,
    low: pd.DataFrame,
    close: pd.DataFrame,
    volume: pd.DataFrame,
    swing_order: int = 5,
    pole_max_days: int = 40,
    flag_max_days: int = 25,
) -> dict[str, pd.DataFrame]:
    """Detect bull flag via impulse (pole) + consolidation (flag).

    For each bar, looks back and:
    1. Finds the most recent swing high (pole top) within flag_max_days
    2. Finds the swing low before that peak (pole start) within pole_max_days
    3. Measures pole size, flag retrace, flag duration, volume ratio

    Returns dict with keys:
      - pole_pct: size of the impulse move (NaN if no pole found)
      - retrace_pct: fraction of the move retraced (0 = at peak, 1 = full retrace)
      - days: duration of flag in bars since pole top
      - vol_ratio: flag volume / pole volume (< 1 = drying up)
    """
    swing_highs, swing_lows = detect_swing_points(high, low, order=swing_order)

    h = high.values.astype(np.float32)
    l = low.values.astype(np.float32)
    c = close.values.astype(np.float32)
    v = volume.values.astype(np.float32)
    sh = swing_highs.values.astype(np.float32)
    sl = swing_lows.values.astype(np.float32)

    try:
        kernel = _make_flag_numba_kernel()
        pp, rp, fd, vr = kernel(h, l, c, v, sh, sl, pole_max_days, flag_max_days)
    except ImportError:
        pp, rp, fd, vr = _flag_kernel_numpy(h, l, c, v, sh, sl, pole_max_days, flag_max_days)

    idx, cols = close.index, close.columns
    return {
        "pole_pct": pd.DataFrame(pp, index=idx, columns=cols),
        "retrace_pct": pd.DataFrame(rp, index=idx, columns=cols),
        "days": pd.DataFrame(fd, index=idx, columns=cols),
        "vol_ratio": pd.DataFrame(vr, index=idx, columns=cols),
    }
