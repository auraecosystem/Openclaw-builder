"""Custom NautilusTrader indicators for Qullamaggie breakout strategy.

Each indicator subclasses nautilus_trader.indicators.Indicator and overrides
handle_bar() to receive OHLCV bars. The primary output is always self.value;
pattern detectors expose additional attributes for multi-dimensional signals.
"""

from collections import deque

import numpy as np

from nautilus_trader.indicators import Indicator
from nautilus_trader.model.data import Bar


# ---------------------------------------------------------------------------
# RollingHigh: 252-bar rolling high (breakout reference level)
# ---------------------------------------------------------------------------

class RollingHigh(Indicator):
    """Track the highest high over a rolling window.

    Uses a monotonic deque for O(1) amortized max tracking instead of
    scanning the full window each bar.
    """

    def __init__(self, period: int = 252):
        super().__init__(params=[period])
        self.period = period
        self._highs: deque[float] = deque(maxlen=period)
        # Monotonic deque: stores (value, index) pairs in decreasing order
        self._max_deque: deque[tuple[float, int]] = deque()
        self.value: float = 0.0
        self._count: int = 0

    def handle_bar(self, bar: Bar) -> None:
        high = bar.high.as_double()
        self._highs.append(high)
        self._count += 1

        # Maintain monotonic deque: remove smaller values from back
        while self._max_deque and self._max_deque[-1][0] <= high:
            self._max_deque.pop()
        self._max_deque.append((high, self._count))

        # Remove expired front entries
        expire = self._count - self.period
        while self._max_deque and self._max_deque[0][1] <= expire:
            self._max_deque.popleft()

        self.value = self._max_deque[0][0]

        if not self.initialized:
            self._set_has_inputs(True)
            if self._count >= self.period:
                self._set_initialized(True)

    def _reset(self) -> None:
        self._highs.clear()
        self._max_deque.clear()
        self.value = 0.0
        self._count = 0


# ---------------------------------------------------------------------------
# RollingReturn: N-bar percentage return of close
# ---------------------------------------------------------------------------

class RollingReturn(Indicator):
    """Percentage return over a rolling window of closes.

    self.value = (current_close / oldest_close) - 1.0
    Useful for momentum ranking (e.g. 21d, 63d, 126d returns).
    """

    def __init__(self, period: int):
        super().__init__(params=[period])
        self.period = period
        # Need period+1 values to compute return over period bars
        self._closes: deque[float] = deque(maxlen=period + 1)
        self.value: float = 0.0

    def handle_bar(self, bar: Bar) -> None:
        close = bar.close.as_double()
        self._closes.append(close)

        if not self.initialized:
            self._set_has_inputs(True)

        if len(self._closes) == self.period + 1:
            oldest = self._closes[0]
            if oldest > 0.0:
                self.value = (close / oldest) - 1.0
            if not self.initialized:
                self._set_initialized(True)

    def _reset(self) -> None:
        self._closes.clear()
        self.value = 0.0


# ---------------------------------------------------------------------------
# ConsecutiveGreen: count of consecutive green (close > open) candles
# ---------------------------------------------------------------------------

class ConsecutiveGreen(Indicator):
    """Count consecutive green candles (close > open).

    Resets to 0 on any red/doji candle. Useful for filtering exhaustion
    moves or confirming momentum thrust.
    """

    def __init__(self):
        super().__init__(params=[])
        self.value: float = 0.0

    def handle_bar(self, bar: Bar) -> None:
        if not self.has_inputs:
            self._set_has_inputs(True)
            self._set_initialized(True)

        if bar.close.as_double() > bar.open.as_double():
            self.value += 1.0
        else:
            self.value = 0.0

    def _reset(self) -> None:
        self.value = 0.0


# ---------------------------------------------------------------------------
# VolumeSMA: simple moving average of volume
# ---------------------------------------------------------------------------

class VolumeSMA(Indicator):
    """SMA of bar volume over a rolling window.

    Uses a running sum for O(1) updates instead of summing the full window.
    """

    def __init__(self, period: int = 20):
        super().__init__(params=[period])
        self.period = period
        self._volumes: deque[float] = deque(maxlen=period)
        self._running_sum: float = 0.0
        self.value: float = 0.0
        self._count: int = 0

    def handle_bar(self, bar: Bar) -> None:
        vol = bar.volume.as_double()

        # Subtract the value that's about to be evicted (if buffer is full)
        if len(self._volumes) == self.period:
            self._running_sum -= self._volumes[0]

        self._volumes.append(vol)
        self._running_sum += vol
        self._count += 1
        self.value = self._running_sum / len(self._volumes)

        if not self.initialized:
            self._set_has_inputs(True)
            if self._count >= self.period:
                self._set_initialized(True)

    def _reset(self) -> None:
        self._volumes.clear()
        self._running_sum = 0.0
        self.value = 0.0
        self._count = 0


# ---------------------------------------------------------------------------
# VCPDetector: Volatility Contraction Pattern via swing-point analysis
# ---------------------------------------------------------------------------

class VCPDetector(Indicator):
    """Detect VCP (Volatility Contraction Pattern) from bar data.

    Uses numpy arrays as the lookback buffer to avoid per-bar list(deque)
    materialization. Swing-point detection stays as a Python loop — on
    60-element arrays, Python loops beat numpy's per-call dispatch overhead.
    Volume averages use numpy slice .sum() for the few calls per _detect.
    """

    def __init__(self, lookback: int = 60, swing_order: int = 5):
        super().__init__(params=[lookback, swing_order])
        self.lookback = lookback
        self.swing_order = swing_order

        # Pre-allocated numpy buffers — avoids list(deque) allocation per bar
        self._buf_h = np.empty(lookback, dtype=np.float64)
        self._buf_l = np.empty(lookback, dtype=np.float64)
        self._buf_c = np.empty(lookback, dtype=np.float64)
        self._buf_v = np.empty(lookback, dtype=np.float64)
        self._n = 0

        # Outputs
        self.value: float = 0.0
        self.num_contractions: int = 0
        self.last_contraction_pct: float = 0.0
        self.tightening_ratio: float = 1.0
        self.vol_trend: float = 1.0

    def handle_bar(self, bar: Bar) -> None:
        h = bar.high.as_double()
        l = bar.low.as_double()
        c = bar.close.as_double()
        v = bar.volume.as_double()

        if self._n < self.lookback:
            self._buf_h[self._n] = h
            self._buf_l[self._n] = l
            self._buf_c[self._n] = c
            self._buf_v[self._n] = v
            self._n += 1
        else:
            # Shift left by 1, append new value at end
            self._buf_h[:-1] = self._buf_h[1:]
            self._buf_l[:-1] = self._buf_l[1:]
            self._buf_c[:-1] = self._buf_c[1:]
            self._buf_v[:-1] = self._buf_v[1:]
            self._buf_h[-1] = h
            self._buf_l[-1] = l
            self._buf_c[-1] = c
            self._buf_v[-1] = v

        if not self.has_inputs:
            self._set_has_inputs(True)

        if self._n < self.lookback:
            return
        if not self.initialized:
            self._set_initialized(True)

        self._detect(c)

    def _detect(self, current_close: float) -> None:
        """Run swing-point detection and contraction counting."""
        n = self._n
        order = self.swing_order

        # Convert to Python lists for the inner loop — numpy scalar access
        # (buf[i] returns np.float64) is slower than Python float access.
        # tolist() is one C call that returns native Python floats.
        highs = self._buf_h[:n].tolist()
        lows = self._buf_l[:n].tolist()

        swing_highs: list[tuple[int, float]] = []
        swing_lows: list[tuple[int, float]] = []

        for i in range(order, n - order):
            hi = highs[i]
            lo = lows[i]
            is_sh = True
            is_sl = True
            for j in range(i - order, i + order + 1):
                if highs[j] > hi:
                    is_sh = False
                if lows[j] < lo:
                    is_sl = False
                if not is_sh and not is_sl:
                    break
            if is_sh:
                swing_highs.append((i, hi))
            if is_sl:
                swing_lows.append((i, lo))

        # Pair each swing high with the next swing low after it
        contractions: list[tuple[float, float, int, int]] = []
        sl_idx = 0
        for sh_bar, sh_price in swing_highs:
            while sl_idx < len(swing_lows) and swing_lows[sl_idx][0] <= sh_bar:
                sl_idx += 1
            if sl_idx < len(swing_lows) and sh_price > 0.0:
                sl_bar, sl_price = swing_lows[sl_idx]
                contractions.append((sh_price, sl_price, sh_bar, sl_bar))

        if not contractions:
            self.num_contractions = 0
            self.last_contraction_pct = 0.0
            self.tightening_ratio = 1.0
            self.vol_trend = 1.0
            self.value = 0.0
            return

        # Compute contraction ranges as fraction of swing high
        ranges = [(c[0] - c[1]) / c[0] for c in contractions]

        # Count progressive contractions (each range < prior)
        progressive = 1
        for i in range(1, len(ranges)):
            if ranges[i] < ranges[i - 1]:
                progressive += 1
            else:
                break

        self.num_contractions = progressive
        self.last_contraction_pct = ranges[-1]
        self.value = float(progressive)

        if len(ranges) >= 2 and ranges[0] > 0.0:
            self.tightening_ratio = ranges[-1] / ranges[0]
        else:
            self.tightening_ratio = 1.0

        # Volume trend: numpy slice mean (only ~2 calls per _detect)
        buf_v = self._buf_v
        c0s, c0e = contractions[0][2], min(contractions[0][3] + 1, n)
        cLs, cLe = contractions[-1][2], min(contractions[-1][3] + 1, n)
        vol_first = float(buf_v[c0s:c0e].mean()) if c0e > c0s else 0.0
        vol_last = float(buf_v[cLs:cLe].mean()) if cLe > cLs else 0.0
        self.vol_trend = (vol_last / vol_first) if vol_first > 0.0 else 1.0

    def _reset(self) -> None:
        self._n = 0
        self.value = 0.0
        self.num_contractions = 0
        self.last_contraction_pct = 0.0
        self.tightening_ratio = 1.0
        self.vol_trend = 1.0


# ---------------------------------------------------------------------------
# FlagDetector: Bull flag (impulse pole + consolidation)
# ---------------------------------------------------------------------------

class FlagDetector(Indicator):
    """Detect bull flag patterns from bar data.

    Same buffer strategy as VCPDetector: numpy arrays avoid per-bar
    list(deque) allocation. Swing detection stays as Python loop.
    """

    def __init__(
        self,
        lookback: int = 60,
        swing_order: int = 5,
        max_flag_bars: int = 25,
        max_pole_bars: int = 20,
    ):
        super().__init__(params=[lookback, swing_order])
        self.lookback = lookback
        self.swing_order = swing_order
        self.max_flag_bars = max_flag_bars
        self.max_pole_bars = max_pole_bars

        # Pre-allocated numpy buffers
        self._buf_h = np.empty(lookback, dtype=np.float64)
        self._buf_l = np.empty(lookback, dtype=np.float64)
        self._buf_c = np.empty(lookback, dtype=np.float64)
        self._buf_v = np.empty(lookback, dtype=np.float64)
        self._n = 0

        # Outputs
        self.value: float = 0.0
        self.pole_pct: float = 0.0
        self.retrace_pct: float = 0.0
        self.flag_days: int = 0
        self.vol_ratio: float = 1.0

    def handle_bar(self, bar: Bar) -> None:
        h = bar.high.as_double()
        l = bar.low.as_double()
        c = bar.close.as_double()
        v = bar.volume.as_double()

        if self._n < self.lookback:
            self._buf_h[self._n] = h
            self._buf_l[self._n] = l
            self._buf_c[self._n] = c
            self._buf_v[self._n] = v
            self._n += 1
        else:
            self._buf_h[:-1] = self._buf_h[1:]
            self._buf_l[:-1] = self._buf_l[1:]
            self._buf_c[:-1] = self._buf_c[1:]
            self._buf_v[:-1] = self._buf_v[1:]
            self._buf_h[-1] = h
            self._buf_l[-1] = l
            self._buf_c[-1] = c
            self._buf_v[-1] = v

        if not self.has_inputs:
            self._set_has_inputs(True)

        if self._n < self.lookback:
            return
        if not self.initialized:
            self._set_initialized(True)

        self._detect()

    def _detect(self) -> None:
        """Find the best pole + flag pattern in the lookback window."""
        n = self._n
        order = self.swing_order
        current_idx = n - 1

        # tolist() for fast Python-native float access in inner loop
        highs = self._buf_h[:n].tolist()
        lows = self._buf_l[:n].tolist()

        swing_highs: list[tuple[int, float]] = []
        swing_lows: list[tuple[int, float]] = []

        for i in range(order, n - order):
            hi = highs[i]
            lo = lows[i]
            is_sh = True
            is_sl = True
            for j in range(i - order, i + order + 1):
                if highs[j] > hi:
                    is_sh = False
                if lows[j] < lo:
                    is_sl = False
                if not is_sh and not is_sl:
                    break
            if is_sh:
                swing_highs.append((i, hi))
            if is_sl:
                swing_lows.append((i, lo))

        # Search for best pole: swing_low -> swing_high with gain >= 20%, bars <= 20
        best_pole_gain = 0.0
        best_pole: tuple[int, float, int, float] | None = None

        for sh_idx, sh_price in swing_highs:
            bars_since_peak = current_idx - sh_idx
            if bars_since_peak < 0 or bars_since_peak > self.max_flag_bars:
                continue

            for sl_idx, sl_price in reversed(swing_lows):
                if sl_idx >= sh_idx:
                    continue
                pole_bars = sh_idx - sl_idx
                if pole_bars > self.max_pole_bars:
                    break
                if sl_price <= 0.0:
                    continue
                gain = (sh_price - sl_price) / sl_price
                if gain >= 0.20 and gain > best_pole_gain:
                    best_pole_gain = gain
                    best_pole = (sl_idx, sl_price, sh_idx, sh_price)
                break

        if best_pole is None:
            self.pole_pct = 0.0
            self.retrace_pct = 0.0
            self.flag_days = 0
            self.vol_ratio = 1.0
            self.value = 0.0
            return

        sl_idx, sl_price, sh_idx, sh_price = best_pole
        pole_gain = (sh_price - sl_price) / sl_price
        move_size = sh_price - sl_price

        self.pole_pct = pole_gain
        self.value = pole_gain
        self.flag_days = current_idx - sh_idx

        current_close = float(self._buf_c[current_idx])
        if move_size > 0.0:
            self.retrace_pct = (sh_price - current_close) / move_size
        else:
            self.retrace_pct = 0.0

        # Volume ratio: numpy slice mean (only 2 calls)
        buf_v = self._buf_v
        pe = min(sh_idx + 1, n)
        vol_pole = float(buf_v[sl_idx:pe].mean()) if pe > sl_idx else 0.0
        fe = min(current_idx + 1, n)
        vol_flag = float(buf_v[sh_idx:fe].mean()) if fe > sh_idx else 0.0
        self.vol_ratio = (vol_flag / vol_pole) if vol_pole > 0.0 else 1.0

    def _reset(self) -> None:
        self._n = 0
        self.value = 0.0
        self.pole_pct = 0.0
        self.retrace_pct = 0.0
        self.flag_days = 0
        self.vol_ratio = 1.0
