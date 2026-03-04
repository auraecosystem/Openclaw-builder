# Agent Report: nautilus-strategy

## Task

Write `/Users/ad/work/ai/openclaw/skills/swingtrader/nautilus/strategy.py` implementing a NautilusTrader strategy for Qullamaggie breakout swing trading across multiple instruments.

## Outcome

File written successfully (346 lines, syntax verified).

## Approach

1. Read the existing custom indicators module at `nautilus/indicators.py` to confirm the API surface (attribute names, constructor signatures) before writing strategy code that depends on them.
2. Wrote the strategy file following the detailed specification, organized into:
   - `QullamaggieConfig` (frozen pydantic/msgspec config)
   - `QullamaggieBreakout` strategy with per-instrument indicator dictionaries
   - Lifecycle methods: `on_start`, `on_bar`, `on_event`, `on_stop`, `on_reset`
   - Private helpers: `_manage_position`, `_evaluate_entry`, `_close_instrument`, `_clear_tracking`
3. Verified syntax with `ast.parse`.

## Key Design Decisions

- Extracted position management and entry evaluation into private methods to keep `on_bar` readable and under the 500 LOC guideline.
- Pattern detection thresholds (VCP: 2+ contractions, tightening < 1, vol_trend < 1; Flag: pole >= 20%, retrace < 50%, 5-25 days, vol_ratio < 1) are inline constants matching the Rust engine defaults.
- Event cleanup uses `isinstance` checks imported lazily inside `on_event` to avoid circular import risk at module level (OrderFilled, PositionClosed are event types only needed for isinstance checks).
- Regime filter is updated on every benchmark bar before any entry logic runs.

## Files

- `/Users/ad/work/ai/openclaw/skills/swingtrader/nautilus/strategy.py` (created, 346 lines)
