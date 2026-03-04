# Agent B Report: execution/ module

## Task

Write the `execution/` module (5 files) for the Rust backtest engine refactor,
extracting and consolidating the three copy-pasted simulation loops from
`src/simulate.rs` into one generic, parameterized simulation function.

## Files Written

All files under `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/execution/`:

1. **`mod.rs`** -- Module declarations and re-exports matching the spec API surface.
2. **`exits.rs`** -- `ExitContext` struct and `ExitRule` enum (5 variants). `should_exit()` dispatches per-variant with direction awareness. `update_stop()` handles the breakeven upgrade pattern.
3. **`position.rs`** -- `PositionSizer` struct with `from_params()` and `compute()`. Consolidates the 10-line position sizing block that was duplicated 3 times in simulate.rs.
4. **`fills.rs`** -- `FillMode` enum (3 variants), `resolve_fill()`, `check_5m_stop()`, and private `find_5m_entry()`. Extracted from simulate.rs's dual-timeframe entry confirmation and intraday stop logic.
5. **`simulator.rs`** -- Generic `simulate()` function with `simulate_ticker()` helper. One loop replaces three (breakout_quick, breakout_runner, ticker_single).

## Design Decisions

### Intraday stop priority
The original code folds the intraday 5m stop into `stop_exit` and uses
`intraday_stop.unwrap_or(close)` for exit price. The refactored simulator checks
the intraday stop as a separate priority path before iterating exit rules. This
produces identical behavior because when the intraday stop fires, it always takes
the 5m exit price (which is what the original does via unwrap_or). The separate
check avoids muddling the ExitRule abstraction with intraday fill details.

### Stop updates before exit checks
`update_stop()` runs for all rules every bar before any `should_exit()` calls.
This matches the original runner logic where breakeven upgrade precedes the exit
condition checks, ensuring the active stop is current when StopLoss evaluates.

### Position sizing caller responsibility
The `PositionSizer::compute()` method uses a `debug_assert` for `price_risk > 0`
rather than returning an error, because the caller (simulator entry block) already
guards `price_risk <= 0.0` with a `continue`. This keeps the hot path branch-free.

### fills.rs reads ConsolHigh from daily store
The original `find_5m_entry` took `consol_high` as a parameter. The refactored
version reads it directly from `store.get(Indicator::ConsolHigh)` to keep the
`resolve_fill()` signature generic across all fill modes.

## Dependencies on Other Agents

- **Agent A (data/)**: imports `crate::data::{DataStore, Indicator}`. Assumes the
  refactored DataStore with `get(Indicator)`, `open()`, `close()`, `intraday` field,
  and `IntradayData` with `matrices` Vec and `day_mapping`.
- **Agent D (types.rs)**: imports `crate::types::{Direction, Params, Trade, ResolvedParams, SignalSet}`.
  These types are unchanged from current types.rs except SignalSet uses `data::WideMask`/`WideMatrix`.
- **Agent C (strategy/)**: the strategy module will call `simulate()` with appropriate
  exit rules, fill mode, direction, and equity fraction per the spec's Setup trait.

## Status

Went according to plan. All 5 files written and cross-checked against the original
simulate.rs for behavioral equivalence. No compilation attempted since the data/
and types modules are not yet written by the other agents.
