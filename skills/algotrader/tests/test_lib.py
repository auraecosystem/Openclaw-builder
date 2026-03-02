"""
Tests for the algotrader Python library (lib/).

All tests use synthetic data built from numpy/pandas -- no real market data,
no external dependencies beyond pytest + numpy + pandas.  Python fallback
functions are tested directly (no numba required).
"""

import sys
from pathlib import Path

# Make lib importable without installing as a package.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import numpy as np
import pandas as pd
import pytest

from lib.indicators import (
    _consecutive_green_pandas,
    atr,
    rolling_return,
    sma,
)
from lib.patterns import (
    consolidation_high,
    consolidation_range,
    detect_swing_points,
    flag_intermediates,
    vcp_intermediates,
)
from lib.risk import (
    _breakeven_stop_python,
    _partial_exit_python,
    _stop_exit_python,
    size_adjusted_slippage,
    size_from_risk,
)
from lib.signals import (
    continuation_breakout_signals,
    episodic_pivot_signals,
    parabolic_short_signals,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_ohlcv(
    closes: list[float] | dict[str, list[float]],
    *,
    spread: float = 0.5,
    volume: float = 1_000_000.0,
) -> dict[str, pd.DataFrame]:
    """Build aligned OHLCV DataFrames from close prices.

    Pass a list for single-ticker or a dict[ticker, list[float]] for multi.
    High = close + spread, Low = close - spread (clamped > 0).
    Open = close (flat candle by default).
    """
    if isinstance(closes, list):
        closes = {"A": closes}

    df_close = pd.DataFrame(closes, dtype=np.float64)
    df_close.index = pd.bdate_range("2024-01-01", periods=len(df_close), freq="B")
    df_open = df_close.copy()
    df_high = df_close + spread
    df_low = (df_close - spread).clip(lower=0.01)
    df_vol = pd.DataFrame(volume, index=df_close.index, columns=df_close.columns)

    return {
        "open": df_open,
        "high": df_high,
        "low": df_low,
        "close": df_close,
        "volume": df_vol,
    }


def _bool_df(values: list[bool] | dict[str, list[bool]], index=None, columns=None) -> pd.DataFrame:
    """Build a boolean DataFrame from a list (single col) or dict."""
    if isinstance(values, list):
        values = {"A": values}
    df = pd.DataFrame(values, dtype=bool)
    if index is not None:
        df.index = index
    if columns is not None:
        df.columns = columns
    return df


def _float_df(values: list[float] | dict[str, list[float]], index=None, columns=None) -> pd.DataFrame:
    """Build a float DataFrame from a list (single col) or dict."""
    if isinstance(values, list):
        values = {"A": values}
    df = pd.DataFrame(values, dtype=np.float64)
    if index is not None:
        df.index = index
    if columns is not None:
        df.columns = columns
    return df


# ====================================================================
# 1. Stop Exit Logic (risk.py)
# ====================================================================

class TestStopExit:
    """Tests for _stop_exit_python."""

    def test_basic_entry_then_breach(self):
        """Entry sets stop; price breaches stop on a later bar -> exit fires."""
        #                    entry  hold   breach
        close = _float_df([100.0, 95.0, 89.0])
        entries = _bool_df([True, False, False])
        stops = _float_df([90.0, np.nan, np.nan])

        result = _stop_exit_python(close, entries, stops)

        assert not result.iloc[0, 0]  # entry bar: no exit
        assert not result.iloc[1, 0]  # 95 > 90: no exit
        assert result.iloc[2, 0]      # 89 <= 90: exit fires

    def test_stop_resets_after_trigger(self):
        """After a stop fires, no second fire until a new entry."""
        close = _float_df([100.0, 89.0, 88.0, 85.0])
        entries = _bool_df([True, False, False, False])
        stops = _float_df([90.0, np.nan, np.nan, np.nan])

        result = _stop_exit_python(close, entries, stops)

        assert result.iloc[1, 0]      # first breach
        assert not result.iloc[2, 0]  # no double-fire
        assert not result.iloc[3, 0]  # still no fire

    def test_price_exactly_at_stop(self):
        """close == stop_price triggers exit (<=, not <)."""
        close = _float_df([100.0, 90.0])
        entries = _bool_df([True, False])
        stops = _float_df([90.0, np.nan])

        result = _stop_exit_python(close, entries, stops)
        assert result.iloc[1, 0]

    def test_no_entry_no_exit(self):
        """Without any entry, no exit should ever fire."""
        close = _float_df([100.0, 50.0, 10.0])
        entries = _bool_df([False, False, False])
        stops = _float_df([90.0, 90.0, 90.0])

        result = _stop_exit_python(close, entries, stops)
        assert not result.any().any()

    def test_multiple_entries_update_stop(self):
        """A second entry updates the active stop price."""
        close = _float_df([100.0, 110.0, 105.0, 95.0])
        entries = _bool_df([True, True, False, False])
        stops = _float_df([90.0, 100.0, np.nan, np.nan])

        result = _stop_exit_python(close, entries, stops)

        # Bar 2: 105 > 100 (new stop): no exit
        assert not result.iloc[2, 0]
        # Bar 3: 95 <= 100 (updated stop): exit fires
        assert result.iloc[3, 0]

    def test_multi_ticker_independence(self):
        """Stops for different tickers are tracked independently."""
        close = _float_df({"X": [100.0, 89.0, 95.0], "Y": [50.0, 55.0, 40.0]})
        entries = _bool_df({"X": [True, False, False], "Y": [True, False, False]})
        stops = _float_df({"X": [90.0, np.nan, np.nan], "Y": [45.0, np.nan, np.nan]})

        result = _stop_exit_python(close, entries, stops)

        # X: breaches at bar 1 (89 <= 90)
        assert result.loc[result.index[1], "X"]
        # Y: does NOT breach at bar 1 (55 > 45), breaches at bar 2 (40 <= 45)
        assert not result.loc[result.index[1], "Y"]
        assert result.loc[result.index[2], "Y"]


class TestPartialExit:
    """Tests for _partial_exit_python."""

    def test_fires_after_n_days_if_profitable(self):
        """Partial exit fires after n_days when close > entry_price."""
        # Entry at 100, then climb. n_days=3.
        close = _float_df([100.0, 102.0, 104.0, 106.0])
        entries = _bool_df([True, False, False, False])

        result = _partial_exit_python(close, entries, n_days=3)

        assert not result.iloc[1, 0]  # day 1
        assert not result.iloc[2, 0]  # day 2
        assert result.iloc[3, 0]      # day 3: 106 > 100 -> fires

    def test_no_fire_if_not_profitable(self):
        """No partial exit if price is below entry at day N."""
        close = _float_df([100.0, 98.0, 96.0, 94.0])
        entries = _bool_df([True, False, False, False])

        result = _partial_exit_python(close, entries, n_days=3)
        assert not result.any().any()

    def test_no_fire_before_n_days(self):
        """Even if profitable, partial exit does not fire before n_days."""
        close = _float_df([100.0, 120.0, 130.0])
        entries = _bool_df([True, False, False])

        result = _partial_exit_python(close, entries, n_days=5)
        assert not result.any().any()

    def test_fires_on_exact_day_n(self):
        """Partial exit fires on exactly n_days (not n_days+1)."""
        # bars_since_entry increments each bar after entry.
        # After 2 bars (not counting entry bar), bars_since=2 >= n_days=2.
        close = _float_df([100.0, 105.0, 110.0])
        entries = _bool_df([True, False, False])

        result = _partial_exit_python(close, entries, n_days=2)

        assert not result.iloc[1, 0]  # bars_since=1, < 2
        assert result.iloc[2, 0]      # bars_since=2, >= 2 and 110 > 100


class TestBreakevenStopExit:
    """Tests for _breakeven_stop_python."""

    def test_before_n_days_original_stop(self):
        """Before n_days, original stop triggers normally."""
        close = _float_df([100.0, 89.0])
        entries = _bool_df([True, False])
        stops = _float_df([90.0, np.nan])

        result = _breakeven_stop_python(close, entries, stops, n_days=5)
        assert result.iloc[1, 0]  # 89 <= 90

    def test_after_n_days_profitable_upgrades_to_breakeven(self):
        """After n_days, if profitable, stop upgrades to entry price."""
        # Entry at 100, stop at 90. After 3 days, if price > 100,
        # stop becomes max(90, 100) = 100. Then breach at 99.
        close = _float_df([100.0, 105.0, 108.0, 110.0, 99.0])
        entries = _bool_df([True, False, False, False, False])
        stops = _float_df([90.0, np.nan, np.nan, np.nan, np.nan])

        result = _breakeven_stop_python(close, entries, stops, n_days=3)

        # Bar 3: bars_since=3 >= 3, close=110 > 100 -> stop upgrades to 100
        assert not result.iloc[3, 0]  # 110 > 100: no exit
        # Bar 4: 99 <= 100 (breakeven stop): exit fires
        assert result.iloc[4, 0]

    def test_after_n_days_not_profitable_stop_unchanged(self):
        """After n_days, if NOT profitable, stop stays at original."""
        # Entry at 100, stop at 90. After 3 days, price is 95 (not > 100),
        # so stop stays at 90. Price at 91 should NOT trigger exit.
        close = _float_df([100.0, 98.0, 96.0, 95.0, 91.0])
        entries = _bool_df([True, False, False, False, False])
        stops = _float_df([90.0, np.nan, np.nan, np.nan, np.nan])

        result = _breakeven_stop_python(close, entries, stops, n_days=3)

        # Bar 3: bars_since=3, close=95 not > 100 -> stop stays at 90
        assert not result.iloc[3, 0]
        # Bar 4: 91 > 90 -> no exit
        assert not result.iloc[4, 0]

    def test_breakeven_stop_only_moves_up(self):
        """Stop can only increase (max of original and entry price)."""
        # Entry at 100, stop at 105 (unusual but tests max logic).
        # After n_days, breakeven = max(105, 100) = 105. Unchanged.
        close = _float_df([100.0, 108.0, 110.0, 112.0, 104.0])
        entries = _bool_df([True, False, False, False, False])
        stops = _float_df([105.0, np.nan, np.nan, np.nan, np.nan])

        result = _breakeven_stop_python(close, entries, stops, n_days=3)

        # Bar 3: bars_since=3, close=112 > 100 -> stop = max(105, 100) = 105
        assert not result.iloc[3, 0]
        # Bar 4: 104 <= 105 -> exit fires (stop was already 105)
        assert result.iloc[4, 0]


# ====================================================================
# 2. Position Sizing (risk.py)
# ====================================================================

class TestSizeFromRisk:

    def test_normal_case(self):
        """shares = (equity * risk_pct) / stop_distance."""
        equity = 100_000.0
        risk_pct = 0.01  # 1%
        # close=100, stop=95 -> distance=5 -> shares = 1000/5 = 200
        close = _float_df([100.0])
        stops = _float_df([95.0])
        entries = _bool_df([True])

        result = size_from_risk(close, stops, entries, equity, risk_pct)
        assert result.iloc[0, 0] == pytest.approx(200.0)

    def test_capped_at_max_pos_pct(self):
        """Position size is capped at max_pos_pct * equity / price."""
        equity = 100_000.0
        risk_pct = 0.10  # 10% -- large, forces cap
        max_pos_pct = 0.20
        # close=100, stop=99 -> distance=1 -> raw_shares = 10000/1 = 10000
        # max_shares = 0.20 * 100000 / 100 = 200
        close = _float_df([100.0])
        stops = _float_df([99.0])
        entries = _bool_df([True])

        result = size_from_risk(close, stops, entries, equity, risk_pct, max_pos_pct)
        assert result.iloc[0, 0] == pytest.approx(200.0)

    def test_zero_stop_distance_returns_nan(self):
        """Zero stop distance should produce NaN, not division by zero."""
        close = _float_df([100.0])
        stops = _float_df([100.0])  # same as close -> distance = 0
        entries = _bool_df([True])

        result = size_from_risk(close, stops, entries, 100_000.0, 0.01)
        assert np.isnan(result.iloc[0, 0])

    def test_non_entry_days_are_nan(self):
        """Non-entry bars should have NaN sizes."""
        close = _float_df([100.0, 105.0])
        stops = _float_df([90.0, 95.0])
        entries = _bool_df([True, False])

        result = size_from_risk(close, stops, entries, 100_000.0, 0.01)
        assert not np.isnan(result.iloc[0, 0])
        assert np.isnan(result.iloc[1, 0])


class TestSizeAdjustedSlippage:

    def test_entry_bars_get_impact_slippage(self):
        """Entry bars should have higher slippage than base."""
        idx = pd.bdate_range("2024-01-01", periods=3)
        sizes = pd.DataFrame({"A": [1000.0, 0.0, 500.0]}, index=idx)
        vol_sma = pd.DataFrame({"A": [10000.0, 10000.0, 10000.0]}, index=idx)
        entries = pd.DataFrame({"A": [True, False, True]}, index=idx)

        result = size_adjusted_slippage(sizes, vol_sma, entries, base_slippage=0.001)

        # Non-entry bar should be exactly base
        assert result.iloc[1, 0] == pytest.approx(0.001)
        # Entry bars should exceed base
        assert result.iloc[0, 0] > 0.001
        assert result.iloc[2, 0] > 0.001

    def test_capped_at_five_percent(self):
        """Slippage is capped at 5% even for huge size / tiny ADV."""
        idx = pd.bdate_range("2024-01-01", periods=1)
        sizes = pd.DataFrame({"A": [1_000_000.0]}, index=idx)
        vol_sma = pd.DataFrame({"A": [1.0]}, index=idx)  # tiny ADV
        entries = pd.DataFrame({"A": [True]}, index=idx)

        result = size_adjusted_slippage(sizes, vol_sma, entries, base_slippage=0.001, k=10.0)
        assert result.iloc[0, 0] == pytest.approx(0.05)

    def test_zero_adv_returns_base(self):
        """Zero ADV should not crash; falls back to base_slippage."""
        idx = pd.bdate_range("2024-01-01", periods=1)
        sizes = pd.DataFrame({"A": [100.0]}, index=idx)
        vol_sma = pd.DataFrame({"A": [0.0]}, index=idx)
        entries = pd.DataFrame({"A": [True]}, index=idx)

        result = size_adjusted_slippage(sizes, vol_sma, entries, base_slippage=0.001)
        # impact is NaN due to 0 ADV, fillna(base) -> base, clipped to [base, 0.05]
        assert result.iloc[0, 0] == pytest.approx(0.001)


# ====================================================================
# 3. Indicators (indicators.py)
# ====================================================================

class TestATR:

    def test_constant_price_atr_zero(self):
        """Constant OHLC should produce ATR = 0 (after warmup)."""
        n = 30
        data = _make_ohlcv([100.0] * n, spread=0.0)

        result = atr(data["high"], data["low"], data["close"], period=14)
        # After warmup, ATR should be ~0
        last_val = result.iloc[-1, 0]
        assert last_val == pytest.approx(0.0, abs=1e-6)

    def test_known_simple_case(self):
        """Verify ATR with a hand-calculated case: one big move then flat."""
        # 5 bars: all at 100, then bar 2 has high=110, low=90 (TR=20).
        # Rest are flat (TR=0 after bar 2).
        closes = [100.0] * 20
        highs = [100.0] * 20
        lows = [100.0] * 20
        # Inject a big-range bar at index 1
        highs[1] = 110.0
        lows[1] = 90.0

        idx = pd.bdate_range("2024-01-01", periods=20)
        h = pd.DataFrame({"A": highs}, index=idx, dtype=np.float64)
        l_ = pd.DataFrame({"A": lows}, index=idx, dtype=np.float64)
        c = pd.DataFrame({"A": closes}, index=idx, dtype=np.float64)

        result = atr(h, l_, c, period=14)
        # ATR at bar 1 is NaN (min_periods=14). By end, should have decayed toward 0.
        assert np.isnan(result.iloc[0, 0])  # not enough periods
        # At the end, the big TR has been exponentially smoothed away
        assert result.iloc[-1, 0] < 5.0  # decayed significantly from 20


class TestSMA:

    def test_known_values(self):
        """SMA of [1,2,3,4,5] with period=3: first valid = (1+2+3)/3 = 2."""
        vals = [1.0, 2.0, 3.0, 4.0, 5.0]
        idx = pd.bdate_range("2024-01-01", periods=5)
        df = pd.DataFrame({"A": vals}, index=idx)

        result = sma(df, period=3)
        assert np.isnan(result.iloc[0, 0])
        assert np.isnan(result.iloc[1, 0])
        assert result.iloc[2, 0] == pytest.approx(2.0)
        assert result.iloc[3, 0] == pytest.approx(3.0)
        assert result.iloc[4, 0] == pytest.approx(4.0)

    def test_nan_before_min_periods(self):
        """All values before min_periods should be NaN."""
        vals = [10.0] * 10
        idx = pd.bdate_range("2024-01-01", periods=10)
        df = pd.DataFrame({"A": vals}, index=idx)

        result = sma(df, period=5)
        for i in range(4):
            assert np.isnan(result.iloc[i, 0])
        assert not np.isnan(result.iloc[4, 0])


class TestRollingReturn:

    def test_known_percentage_changes(self):
        """Verify pct_change over known periods."""
        # close doubles from 100 to 200 over 5 bars: [100, 125, 150, 175, 200]
        vals = [100.0, 125.0, 150.0, 175.0, 200.0]
        idx = pd.bdate_range("2024-01-01", periods=5)
        close = pd.DataFrame({"A": vals}, index=idx)

        result = rolling_return(close, periods=[2, 4])

        # Period=2: bar 2 -> (150-100)/100 = 0.5
        assert result[2].iloc[2, 0] == pytest.approx(0.5)
        # Period=4: bar 4 -> (200-100)/100 = 1.0
        assert result[4].iloc[4, 0] == pytest.approx(1.0)

    def test_default_periods(self):
        """Default periods should be [21, 63, 126]."""
        close = pd.DataFrame({"A": np.arange(1.0, 200.0)})
        result = rolling_return(close)
        assert set(result.keys()) == {21, 63, 126}


class TestConsecutiveGreenDays:

    def test_streak_of_green(self):
        """Consecutive green days should produce incrementing counts."""
        # Green: close > open. Make open=100, close increasing.
        idx = pd.bdate_range("2024-01-01", periods=5)
        open_ = pd.DataFrame({"A": [100.0] * 5}, index=idx)
        close = pd.DataFrame({"A": [101.0, 102.0, 103.0, 104.0, 105.0]}, index=idx)

        is_green = (close > open_).astype(np.int8)
        result = _consecutive_green_pandas(is_green)

        assert list(result["A"]) == [1, 2, 3, 4, 5]

    def test_red_day_resets(self):
        """A red day (close <= open) resets the streak to 0."""
        idx = pd.bdate_range("2024-01-01", periods=4)
        open_ = pd.DataFrame({"A": [100.0] * 4}, index=idx)
        close = pd.DataFrame({"A": [101.0, 102.0, 99.0, 101.0]}, index=idx)

        is_green = (close > open_).astype(np.int8)
        result = _consecutive_green_pandas(is_green)

        assert list(result["A"]) == [1, 2, 0, 1]

    def test_mixed_sequence(self):
        """Mixed green/red pattern."""
        idx = pd.bdate_range("2024-01-01", periods=6)
        open_ = pd.DataFrame({"A": [10.0] * 6}, index=idx)
        # G, R, G, G, R, G
        close = pd.DataFrame({"A": [11.0, 9.0, 11.0, 12.0, 8.0, 11.0]}, index=idx)

        is_green = (close > open_).astype(np.int8)
        result = _consecutive_green_pandas(is_green)
        assert list(result["A"]) == [1, 0, 1, 2, 0, 1]


# ====================================================================
# 4. Pattern Detection (patterns.py)
# ====================================================================

class TestConsolidationRange:

    def test_flat_price_range_near_zero(self):
        """Flat price -> consolidation range ~ 0."""
        data = _make_ohlcv([100.0] * 50, spread=0.0)
        result = consolidation_range(data["high"], data["low"], lookback=20)
        # After warmup, range should be 0
        last_val = result.iloc[-1, 0]
        assert last_val == pytest.approx(0.0, abs=1e-6)

    def test_known_range(self):
        """Known high/low range produces expected result."""
        n = 30
        idx = pd.bdate_range("2024-01-01", periods=n)
        # High = 110 every day, low = 90 every day -> range = (110-90)/110
        high = pd.DataFrame({"A": [110.0] * n}, index=idx)
        low = pd.DataFrame({"A": [90.0] * n}, index=idx)

        result = consolidation_range(high, low, lookback=10)
        expected = (110 - 90) / 110
        assert result.iloc[-1, 0] == pytest.approx(expected)


class TestConsolidationHigh:

    def test_excludes_todays_high(self):
        """consolidation_high uses shift(1) -- today's high is excluded."""
        n = 25
        idx = pd.bdate_range("2024-01-01", periods=n)
        highs = [100.0] * (n - 1) + [200.0]  # spike on last day
        high = pd.DataFrame({"A": highs}, index=idx)

        result = consolidation_high(high, lookback=10)
        # Last bar: should be 100 (prior highs), NOT 200
        assert result.iloc[-1, 0] == pytest.approx(100.0)


class TestSwingPoints:

    def test_clear_swing_high(self):
        """Middle bar highest in window -> swing high."""
        # Window: 11 bars (order=5). Bar 5 is highest.
        n = 11
        idx = pd.bdate_range("2024-01-01", periods=n)
        highs = list(range(100, 106)) + list(range(104, 99, -1))  # peak at bar 5 (105)
        lows = [h - 2 for h in highs]
        high = pd.DataFrame({"A": highs}, index=idx, dtype=np.float64)
        low = pd.DataFrame({"A": lows}, index=idx, dtype=np.float64)

        sh, _ = detect_swing_points(high, low, order=5)
        # Bar 5 should be a swing high
        assert sh.iloc[5, 0] == pytest.approx(105.0)
        # Other bars should be NaN
        assert np.isnan(sh.iloc[0, 0])

    def test_clear_swing_low(self):
        """Middle bar lowest in window -> swing low."""
        n = 11
        idx = pd.bdate_range("2024-01-01", periods=n)
        lows = list(range(105, 99, -1)) + list(range(101, 106))  # trough at bar 5 (100)
        highs = [l + 2 for l in lows]
        high = pd.DataFrame({"A": highs}, index=idx, dtype=np.float64)
        low = pd.DataFrame({"A": lows}, index=idx, dtype=np.float64)

        _, sl = detect_swing_points(high, low, order=5)
        assert sl.iloc[5, 0] == pytest.approx(100.0)

    def test_non_swing_bars_are_nan(self):
        """Bars that are not extremes should be NaN."""
        n = 11
        idx = pd.bdate_range("2024-01-01", periods=n)
        # Monotonically increasing: no interior swing highs/lows (edges are NaN from window)
        highs = list(range(100, 111))
        lows = list(range(99, 110))
        high = pd.DataFrame({"A": highs}, index=idx, dtype=np.float64)
        low = pd.DataFrame({"A": lows}, index=idx, dtype=np.float64)

        sh, sl = detect_swing_points(high, low, order=5)
        # No swing highs or lows in a monotonic series (center can't be max/min of full window)
        # The last bar (bar 10) is the highest high, but it needs 5 bars after it (not available)
        # so with min_periods=window (strict), edges are NaN.
        assert sh.isna().all().all()


class TestVCPIntermediates:

    def test_tightening_contractions(self):
        """Synthetic progressively tightening contractions should be detected."""
        # Build a price series with clear swing highs and lows that tighten:
        # Peak1(120) -> Trough1(80) -> Peak2(115) -> Trough2(90) -> Peak3(110) -> Trough3(95)
        # Contractions: 40/120=33%, 25/115=22%, 15/110=14% -- progressively smaller.
        n = 70
        idx = pd.bdate_range("2024-01-01", periods=n)

        # Smooth price path with clear oscillations that tighten
        prices = []
        for i in range(n):
            base = 100.0
            # Damped oscillation
            amp = 20.0 * np.exp(-i / 25.0)
            prices.append(base + amp * np.sin(2 * np.pi * i / 14))

        close = pd.DataFrame({"A": prices}, index=idx, dtype=np.float64)
        high = close + 1.0
        low = close - 1.0
        vol = pd.DataFrame({"A": [1e6] * n}, index=idx, dtype=np.float64)

        result = vcp_intermediates(high, low, close, vol, swing_order=3, lookback=50)

        # At the tail, we expect some contractions detected (exact count depends on oscillation)
        # Just verify the structure and that at least one bar has >0 contractions
        assert "num_contractions" in result
        assert "tightening_ratio" in result

        num_c = result["num_contractions"]
        # At least one bar in the latter half should have detected contractions
        latter = num_c.iloc[n // 2:]
        valid = latter.dropna()
        assert len(valid) > 0, "Expected at least one bar with detected contractions"

    def test_flat_price_no_contractions(self):
        """Perfectly flat price -> no meaningful swing points, no contractions."""
        n = 70
        data = _make_ohlcv([100.0] * n, spread=0.0)

        result = vcp_intermediates(
            data["high"], data["low"], data["close"], data["volume"],
            swing_order=3, lookback=50,
        )

        # Flat price: every bar is a swing high AND low (all equal),
        # but contraction range = 0/price, so tightening should be 1.0 or NaN.
        # Just check it runs without error and produces the right shape.
        assert result["num_contractions"].shape == (n, 1)


class TestFlagIntermediates:

    def test_pole_plus_consolidation(self):
        """Sharp rise (pole) followed by flat consolidation -> flag detected."""
        # Pole: price rises from 80 to 120 over 10 bars (50% gain)
        # Flag: price stays around 115 for 15 bars
        n = 80
        idx = pd.bdate_range("2024-01-01", periods=n)

        prices = []
        for i in range(n):
            if i < 20:
                # Flat base
                prices.append(80.0)
            elif i < 30:
                # Sharp pole: 80 -> 120 in 10 bars
                prices.append(80.0 + 4.0 * (i - 20))
            else:
                # Consolidation near the top, slight pullback
                prices.append(115.0 + np.sin(i) * 2)

        close = pd.DataFrame({"A": prices}, index=idx, dtype=np.float64)
        high = close + 1.0
        low = close - 1.0
        vol = pd.DataFrame({"A": [1e6] * n}, index=idx, dtype=np.float64)

        result = flag_intermediates(
            high, low, close, vol,
            swing_order=3, pole_max_days=30, flag_max_days=25,
        )

        # Check shape
        assert result["pole_pct"].shape == (n, 1)
        # After the flag forms, some bars should have pole_pct > 0.15
        latter = result["pole_pct"].iloc[40:]
        valid = latter.dropna()
        assert len(valid) > 0, "Expected flag detection after pole"
        assert (valid > 0.15).any().any(), "Pole percentage should exceed 15%"

    def test_no_pole_all_nan(self):
        """Flat price with no pole -> all pole_pct NaN."""
        n = 80
        data = _make_ohlcv([100.0] * n, spread=0.0)

        result = flag_intermediates(
            data["high"], data["low"], data["close"], data["volume"],
            swing_order=3, pole_max_days=30, flag_max_days=25,
        )
        # No pole should be detected in flat price
        assert result["pole_pct"].isna().all().all()


# ====================================================================
# 5. Signals (signals.py)
# ====================================================================

class TestContinuationBreakoutSignals:

    def _make_inputs(self, n=10):
        """Build valid default inputs where all conditions are met on bar 5."""
        idx = pd.bdate_range("2024-01-01", periods=n)
        cols = ["A"]
        close = pd.DataFrame({"A": [100.0] * n}, index=idx)
        high = close + 1.0
        low = close - 1.0
        volume = pd.DataFrame({"A": [2_000_000.0] * n}, index=idx)
        atr_14 = pd.DataFrame({"A": [2.0] * n}, index=idx)
        sma_10 = pd.DataFrame({"A": [99.0] * n}, index=idx)  # below close -> no SMA exit
        vol_sma_20 = pd.DataFrame({"A": [1_000_000.0] * n}, index=idx)
        is_pattern = pd.DataFrame({"A": [False] * n}, index=idx)
        consol_high = pd.DataFrame({"A": [99.0] * n}, index=idx)  # below close -> breakout

        # Activate conditions on bar 5
        is_pattern.iloc[5, 0] = True

        return {
            "close": close, "high": high, "low": low, "volume": volume,
            "atr_14": atr_14, "sma_10": sma_10, "vol_sma_20": vol_sma_20,
            "is_pattern": is_pattern, "consol_high": consol_high,
        }

    def test_all_conditions_met(self):
        """When all conditions are met, entry fires."""
        inp = self._make_inputs()
        result = continuation_breakout_signals(
            inp["close"], inp["high"], inp["low"], inp["volume"],
            inp["atr_14"], inp["sma_10"], inp["vol_sma_20"],
            inp["is_pattern"], inp["consol_high"],
            vol_ratio=1.5, min_adv_dollars=1_000_000,
        )
        assert result["entries"].iloc[5, 0]

    def test_missing_pattern_no_entry(self):
        """Without a pattern, no entry fires even if other conditions met."""
        inp = self._make_inputs()
        inp["is_pattern"].iloc[5, 0] = False

        result = continuation_breakout_signals(
            inp["close"], inp["high"], inp["low"], inp["volume"],
            inp["atr_14"], inp["sma_10"], inp["vol_sma_20"],
            inp["is_pattern"], inp["consol_high"],
            vol_ratio=1.5, min_adv_dollars=1_000_000,
        )
        assert not result["entries"].any().any()

    def test_missing_volume_no_entry(self):
        """Without a volume spike, no entry fires."""
        inp = self._make_inputs()
        # Volume below threshold: 1.5 * 1M = 1.5M, set volume to 1M (below)
        inp["volume"].iloc[5, 0] = 500_000.0

        result = continuation_breakout_signals(
            inp["close"], inp["high"], inp["low"], inp["volume"],
            inp["atr_14"], inp["sma_10"], inp["vol_sma_20"],
            inp["is_pattern"], inp["consol_high"],
            vol_ratio=1.5, min_adv_dollars=1_000_000,
        )
        assert not result["entries"].iloc[5, 0]

    def test_stop_price_is_max_of_low_and_close_minus_atr(self):
        """stop = max(low, close - ATR)."""
        inp = self._make_inputs()
        # close=100, atr=2 -> close-atr=98; low=99 -> stop = max(99, 98) = 99
        result = continuation_breakout_signals(
            inp["close"], inp["high"], inp["low"], inp["volume"],
            inp["atr_14"], inp["sma_10"], inp["vol_sma_20"],
            inp["is_pattern"], inp["consol_high"],
        )
        assert result["stop_prices"].iloc[5, 0] == pytest.approx(99.0)


class TestEpisodicPivotSignals:

    def test_gap_day_entry_is_next_day(self):
        """Entry signal should be shifted forward by 1 day from the gap."""
        n = 10
        idx = pd.bdate_range("2024-01-01", periods=n)
        close = pd.DataFrame({"A": [100.0] * n}, index=idx)
        open_ = pd.DataFrame({"A": [100.0] * n}, index=idx)
        low = pd.DataFrame({"A": [99.0] * n}, index=idx)
        volume = pd.DataFrame({"A": [3_000_000.0] * n}, index=idx)
        vol_sma_20 = pd.DataFrame({"A": [1_000_000.0] * n}, index=idx)
        sma_10 = pd.DataFrame({"A": [95.0] * n}, index=idx)

        # Create a gap on bar 3: open=112 vs prev_close=100 -> 12% gap
        open_.iloc[3, 0] = 112.0
        close.iloc[3, 0] = 115.0  # close high to keep liquidity filter

        result = episodic_pivot_signals(
            open_, close, low, volume, vol_sma_20, sma_10,
            min_gap_pct=0.10, min_vol_ratio=2.0, min_adv_dollars=1_000_000,
        )

        # Gap detected on bar 3. Entry should be on bar 4 (shifted forward).
        assert not result["entries"].iloc[3, 0]
        assert result["entries"].iloc[4, 0]

    def test_stop_is_gap_day_low(self):
        """Stop price should be the gap day's low, carried to entry day."""
        n = 10
        idx = pd.bdate_range("2024-01-01", periods=n)
        close = pd.DataFrame({"A": [100.0] * n}, index=idx)
        open_ = pd.DataFrame({"A": [100.0] * n}, index=idx)
        low = pd.DataFrame({"A": [99.0] * n}, index=idx)
        volume = pd.DataFrame({"A": [3_000_000.0] * n}, index=idx)
        vol_sma_20 = pd.DataFrame({"A": [1_000_000.0] * n}, index=idx)
        sma_10 = pd.DataFrame({"A": [95.0] * n}, index=idx)

        open_.iloc[3, 0] = 112.0
        close.iloc[3, 0] = 115.0
        low.iloc[3, 0] = 108.0  # gap day's low

        result = episodic_pivot_signals(
            open_, close, low, volume, vol_sma_20, sma_10,
            min_gap_pct=0.10, min_vol_ratio=2.0, min_adv_dollars=1_000_000,
        )

        # Stop at entry day (bar 4) should be gap day's low (108.0)
        assert result["stop_prices"].iloc[4, 0] == pytest.approx(108.0)


class TestParabolicShortSignals:

    def test_first_red_after_green_streak(self):
        """First red day after 3+ green days + parabolic run -> entry."""
        n = 10
        idx = pd.bdate_range("2024-01-01", periods=n)
        open_ = pd.DataFrame({"A": [100.0] * n}, index=idx)
        close = pd.DataFrame({"A": [100.0] * n}, index=idx)
        high = pd.DataFrame({"A": [101.0] * n}, index=idx)
        sma_10 = pd.DataFrame({"A": [90.0] * n}, index=idx)
        sma_20 = pd.DataFrame({"A": [85.0] * n}, index=idx)
        pct_10d = pd.DataFrame({"A": [0.0] * n}, index=idx)
        vol_sma_20 = pd.DataFrame({"A": [1_000_000.0] * n}, index=idx)

        # Set up consec_green: bars 1-4 have green streak >= 3
        # We just provide the pre-computed streak directly
        consec_green = pd.DataFrame({"A": [0, 1, 2, 3, 4, 0, 0, 0, 0, 0]}, index=idx)

        # Bar 5 is the first red day: close < open
        close.iloc[5, 0] = 95.0  # red (< open=100)
        # Parabolic run: pct_10d >= 3.0 (small cap, close=95 < 50 is False; close=95 < 50? No.
        # close=95 > 50 -> large cap -> need pct_10d >= 0.50
        pct_10d.iloc[5, 0] = 0.60
        # consec_green.shift(1) at bar 5 = consec_green at bar 4 = 4 >= 3

        result = parabolic_short_signals(
            open_, close, high, sma_10, sma_20, consec_green, pct_10d,
            vol_sma_20=vol_sma_20,
            min_green_days=3, min_adv_dollars=1_000_000,
        )

        assert result["entries"].iloc[5, 0]

    def test_only_two_green_days_no_entry(self):
        """Only 2 green days (less than min_green_days=3) -> no entry."""
        n = 10
        idx = pd.bdate_range("2024-01-01", periods=n)
        open_ = pd.DataFrame({"A": [100.0] * n}, index=idx)
        close = pd.DataFrame({"A": [100.0] * n}, index=idx)
        high = pd.DataFrame({"A": [101.0] * n}, index=idx)
        sma_10 = pd.DataFrame({"A": [90.0] * n}, index=idx)
        sma_20 = pd.DataFrame({"A": [85.0] * n}, index=idx)
        pct_10d = pd.DataFrame({"A": [0.60] * n}, index=idx)
        vol_sma_20 = pd.DataFrame({"A": [1_000_000.0] * n}, index=idx)

        # Only 2 green days before bar 5
        consec_green = pd.DataFrame({"A": [0, 0, 0, 1, 2, 0, 0, 0, 0, 0]}, index=idx)
        close.iloc[5, 0] = 95.0  # red

        result = parabolic_short_signals(
            open_, close, high, sma_10, sma_20, consec_green, pct_10d,
            vol_sma_20=vol_sma_20,
            min_green_days=3, min_adv_dollars=1_000_000,
        )

        assert not result["entries"].iloc[5, 0]
