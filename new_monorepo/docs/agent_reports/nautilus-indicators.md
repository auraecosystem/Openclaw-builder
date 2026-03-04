# Agent Report: NautilusTrader Custom Indicators

## Task
Create `/Users/ad/work/ai/openclaw/skills/swingtrader/nautilus/indicators.py` containing six custom NautilusTrader `Indicator` subclasses for the Qullamaggie breakout strategy.

## Result
File created successfully at 437 lines. Python syntax verified.

## Indicators Implemented

1. **RollingHigh** -- 252-bar rolling max of bar.high. Uses deque(maxlen=period).
2. **RollingReturn** -- N-bar close-to-close return. deque(maxlen=period+1), value = (current/oldest) - 1.
3. **ConsecutiveGreen** -- Stateless counter of consecutive close > open candles. Initialized immediately.
4. **VolumeSMA** -- SMA of bar volume via deque accumulation.
5. **VCPDetector** -- Volatility Contraction Pattern. Swing-point detection within lookback buffer, contraction pairing, progressive contraction counting. Exposes num_contractions, last_contraction_pct, tightening_ratio, vol_trend.
6. **FlagDetector** -- Bull flag pattern. Finds pole (swing_low to swing_high with >= 20% gain, <= 20 bars) then measures flag retrace, duration, and volume ratio.

## Approach

- Studied the canonical NautilusTrader Python indicator example (`ema_python.py`) for the exact subclassing protocol: `super().__init__(params=[...])`, `_set_has_inputs(True)`, `_set_initialized(True)`, `handle_bar()`, `_reset()`.
- Ported VCP and Flag detection logic from the existing `skills/swingtrader/lib/patterns.py` (which operates on wide DataFrames) into a single-bar streaming form using deque buffers.
- Pattern detectors convert the deque to a list each bar and run swing-point detection inline. This is O(lookback * swing_order) per bar -- acceptable for live trading tick rates but would need profiling for large backtests.

## Deviations
None. Plan executed as specified.
