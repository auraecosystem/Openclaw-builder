# Agent D: Glue Files for Engine Refactor

## Summary

Wrote 8 files that wire together the new module structure for the
swingtrader-engine Rust refactor.  Also updated `cli.rs` to reference
`swingtrader_engine::types::Params` (binary crate cannot use `crate::types`
after the lib/bin split).

## Files Written

| File | Action | Lines |
|------|--------|-------|
| `src/types.rs` | REWRITE -- removed WideMatrix/WideMask/Axes/DataStore, kept Params/Trade/Direction/SignalSet/ResolvedParams, added `use crate::data::{WideMask, WideMatrix}` | 138 |
| `src/lib.rs` | NEW -- crate root with module declarations, `run_single()`, `run_batch()` | 94 |
| `src/main.rs` | REWRITE -- slim CLI dispatch calling `swingtrader_engine::*` | 44 |
| `src/analysis/mod.rs` | NEW -- re-exports metrics + report | 6 |
| `src/analysis/metrics.rs` | COPY verbatim from `src/metrics.rs` | 127 |
| `src/analysis/report.rs` | ADAPT from `src/report.rs` -- changed `crate::types::Axes` to `crate::data::Axes`, `crate::metrics` to `super::metrics` | 167 |
| `src/server/mod.rs` | NEW -- re-exports `serve` | 5 |
| `src/server/tcp.rs` | EXTRACT from old `main.rs` `serve_mode()`, adapted imports to new module paths | 110 |

## Additional Changes

| File | Change |
|------|--------|
| `src/cli.rs` | `crate::types::Params` -> `swingtrader_engine::types::Params` (2 occurrences) since cli.rs is now part of the binary crate, not the library |

## Coordination With Other Agents

- Agent A had already written `data/indicator.rs`, `data/matrix.rs`, `data/store.rs`
  (missing: `data/mod.rs`, `data/loader.rs`)
- Agent B had already written `execution/exits.rs`, `execution/position.rs`
  (missing: `execution/mod.rs`, `execution/fills.rs`, `execution/simulator.rs`)
- Agent C had already written `strategy/mod.rs`
  (missing: `strategy/breakout.rs`, `strategy/ep.rs`, `strategy/parabolic.rs`)

The glue files (`lib.rs`, `main.rs`, module re-exports) assume those
missing files will be created by their respective agents.  The `lib.rs`
`run_single()` function follows the exact API contracts specified in
`REFACTOR_SPEC.md`.

## Design Decisions

1. **lib.rs `run_single` uses trait dispatch** -- iterates `create_setups()`
   instead of a match statement, matching the spec's goal of replacing
   hardcoded setup dispatch with the `Setup` trait.

2. **server/tcp.rs calls `crate::run_single`** -- the TCP server delegates
   to the library's `run_single` (same as the old `batch::run_single`),
   preserving the existing progress-reporting thread pattern.

3. **Unused import hygiene** -- only imported types that are directly named
   in function signatures or bodies (`Report`, `DataStore`, `Params`, etc.).
   Types used implicitly through method return types (e.g. `ExitRule` from
   `setup.exit_rules()`) are not imported since they would trigger warnings.

## Outcome

All 8 assigned files written.  The crate will not compile standalone until
agents A/B/C finish their remaining files (`data/mod.rs`, `data/loader.rs`,
`execution/mod.rs`, `execution/fills.rs`, `execution/simulator.rs`,
`strategy/breakout.rs`, `strategy/ep.rs`, `strategy/parabolic.rs`).
