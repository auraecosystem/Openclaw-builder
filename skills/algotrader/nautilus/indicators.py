"""Custom NautilusTrader indicators for Qullamaggie breakout strategy.

Each indicator subclasses nautilus_trader.indicators.Indicator and overrides
handle_bar() to receive OHLCV bars. The primary output is always self.value;
pattern detectors expose additional attributes for multi-dimensional signals.
"""

from collections import deque

from nautilus_trader.indicators import Indicator
from nautilus_trader.model.data import Bar


# ---------------------------------------------------------------------------
# RollingHigh: 252-bar rolling high (breakout reference level)
# ---------------------------------------------------------------------------

class RollingHigh(Indicator):
    """Track the highest high over a rolling window.

    Used to identify 52-week breakout levels. self.value holds the current
    rolling max of bar highs.
    """

    def __init__(self, period: int = 252):
        super().__init__(params=[period])
        self.period = period
        self._highs: deque[float] = deque(maxlen=period)
        self.value: float = 0.0
        self._count: int = 0

    def handle_bar(self, bar: Bar) -> None:
        high = bar.high.as_double()
        self._highs.append(high)
        self._count += 1
        self.value = max(self._highs)

        if not self.initialized:
            self._set_has_inputs(True)
            if self._count >= self.period:
                self._set_initialized(True)

    def _reset(self) -> None:
        self._highs.clear()
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

    The built-in SMA only handles price via update_raw; this variant
    extracts volume from the full bar.
    """

    def __init__(self, period: int = 20):
        super().__init__(params=[period])
        self.period = period
        self._volumes: deque[float] = deque(maxlen=period)
        self.value: float = 0.0
        self._count: int = 0

    def handle_bar(self, bar: Bar) -> None:
        vol = bar.volume.as_double()
        self._volumes.append(vol)
        self._count += 1
        self.value = sum(self._volumes) / len(self._volumes)

        if not self.initialized:
            self._set_has_inputs(True)
            if self._count >= self.period:
                self._set_initialized(True)

    def _reset(self) -> None:
        self._volumes.clear()
        self.value = 0.0
        self._count = 0


# ---------------------------------------------------------------------------
# VCPDetector: Volatility Contraction Pattern via swing-point analysis
# ---------------------------------------------------------------------------

class VCPDetector(Indicator):
    """Detect VCP (Volatility Contraction Pattern) from bar data.

    Maintains a lookback buffer of OHLCV data. On each bar, identifies
    swing highs/lows, pairs them into contractions, and counts progressive
    contractions where each range is smaller than the prior.

    Primary output (self.value) is the contraction count. Additional
    attributes provide tightening quality metrics.
    """

    def __init__(self, lookback: int = 60, swing_order: int = 5):
        super().__init__(params=[lookback, swing_order])
        self.lookback = lookback
        self.swing_order = swing_order
        self._bars: deque[tuple[float, float, float, float]] = deque(maxlen=lookback)

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
        self._bars.append((h, l, c, v))

        if not self.has_inputs:
            self._set_has_inputs(True)

        n = len(self._bars)
        if n < self.lookback:
            return
        if not self.initialized:
            self._set_initialized(True)

        self._detect(c)

    def _detect(self, current_close: float) -> None:
        """Run swing-point detection and contraction counting."""
        bars = list(self._bars)
        n = len(bars)
        order = self.swing_order

        # Identify swing highs and swing lows
        # A swing high at i: bars[i].high is the max high in [i-order, i+order]
        swing_highs: list[tuple[int, float]] = []
        swing_lows: list[tuple[int, float]] = []

        for i in range(order, n - order):
            hi = bars[i][0]
            lo = bars[i][1]
            is_sh = True
            is_sl = True
            for j in range(i - order, i + order + 1):
                if bars[j][0] > hi:
                    is_sh = False
                if bars[j][1] < lo:
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

        # Volume trend: avg volume in last contraction vs first
        vol_first = self._avg_vol(bars, contractions[0][2], contractions[0][3])
        vol_last = self._avg_vol(bars, contractions[-1][2], contractions[-1][3])
        self.vol_trend = (vol_last / vol_first) if vol_first > 0.0 else 1.0

    @staticmethod
    def _avg_vol(
        bars: list[tuple[float, float, float, float]],
        start: int,
        end: int,
    ) -> float:
        """Average volume between bar indices [start, end] inclusive."""
        total = 0.0
        count = 0
        for i in range(start, min(end + 1, len(bars))):
            total += bars[i][3]
            count += 1
        return total / count if count > 0 else 0.0

    def _reset(self) -> None:
        self._bars.clear()
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

    Searches for a strong impulse move (pole) followed by a tight
    consolidation (flag). The pole is identified by a swing_low -> swing_high
    pair with sufficient gain over a short duration.

    Primary output (self.value) is the pole percentage gain. Additional
    attributes describe flag quality.
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
        self._bars: deque[tuple[float, float, float, float]] = deque(maxlen=lookback)

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
        self._bars.append((h, l, c, v))

        if not self.has_inputs:
            self._set_has_inputs(True)

        n = len(self._bars)
        if n < self.lookback:
            return
        if not self.initialized:
            self._set_initialized(True)

        self._detect()

    def _detect(self) -> None:
        """Find the best pole + flag pattern in the lookback window."""
        bars = list(self._bars)
        n = len(bars)
        order = self.swing_order
        current_idx = n - 1

        # Identify swing highs and swing lows
        swing_highs: list[tuple[int, float]] = []
        swing_lows: list[tuple[int, float]] = []

        for i in range(order, n - order):
            hi = bars[i][0]
            lo = bars[i][1]
            is_sh = True
            is_sl = True
            for j in range(i - order, i + order + 1):
                if bars[j][0] > hi:
                    is_sh = False
                if bars[j][1] < lo:
                    is_sl = False
                if not is_sh and not is_sl:
                    break
            if is_sh:
                swing_highs.append((i, hi))
            if is_sl:
                swing_lows.append((i, lo))

        # Search for best pole: swing_low -> swing_high with gain >= 20%, bars <= 20
        best_pole_gain = 0.0
        best_pole: tuple[int, float, int, float] | None = None  # (sl_idx, sl_price, sh_idx, sh_price)

        for sh_idx, sh_price in swing_highs:
            # Only consider peaks recent enough to form a flag to the current bar
            bars_since_peak = current_idx - sh_idx
            if bars_since_peak < 0 or bars_since_peak > self.max_flag_bars:
                continue

            # Find the swing low before this peak
            for sl_idx, sl_price in reversed(swing_lows):
                if sl_idx >= sh_idx:
                    continue
                pole_bars = sh_idx - sl_idx
                if pole_bars > self.max_pole_bars:
                    break  # too far back
                if sl_price <= 0.0:
                    continue
                gain = (sh_price - sl_price) / sl_price
                if gain >= 0.20 and gain > best_pole_gain:
                    best_pole_gain = gain
                    best_pole = (sl_idx, sl_price, sh_idx, sh_price)
                break  # only check the nearest swing low

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

        # Retrace: how much of the pole move given back at current close
        current_close = bars[current_idx][2]
        if move_size > 0.0:
            self.retrace_pct = (sh_price - current_close) / move_size
        else:
            self.retrace_pct = 0.0

        # Volume ratio: avg volume in flag / avg volume in pole
        vol_pole = self._avg_vol(bars, sl_idx, sh_idx)
        vol_flag = self._avg_vol(bars, sh_idx, current_idx)
        self.vol_ratio = (vol_flag / vol_pole) if vol_pole > 0.0 else 1.0

    @staticmethod
    def _avg_vol(
        bars: list[tuple[float, float, float, float]],
        start: int,
        end: int,
    ) -> float:
        """Average volume between bar indices [start, end] inclusive."""
        total = 0.0
        count = 0
        for i in range(start, min(end + 1, len(bars))):
            total += bars[i][3]
            count += 1
        return total / count if count > 0 else 0.0

    def _reset(self) -> None:
        self._bars.clear()
        self.value = 0.0
        self.pole_pct = 0.0
        self.retrace_pct = 0.0
        self.flag_days = 0
        self.vol_ratio = 1.0
