# Agent Report: --compare Flag CLI Addition

## Task

Add `--compare` flag to the CLI (Agent 1 of 3 in the parity smoke test plan). Owned files: `src/cli.rs` and `src/main.rs`.

## Changes Made

### `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/cli.rs`

Added `compare: Option<String>` field to the `Cli` struct, placed immediately before `dump_trades`:

```rust
/// Compare hardcoded strategy against dynamic JSON equivalent.
/// Use "all" to compare all strategy pairs, or a specific pair like "ep:ep_dynamic"
#[arg(long)]
pub compare: Option<String>,
```

### `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/main.rs`

Inserted a new dispatch branch before the `dump_trades` branch. Handles two sub-cases:

- `--compare all`: calls `compare_all()`, prints JSON array of all `CompareResult`s
- `--compare hardcoded:dynamic` (e.g. `ep:ep_dynamic`): calls `compare_strategies()`, prints a single `CompareResult` as JSON
- Malformed format bails with a clear error message via `anyhow::bail!`

## Verification

`cargo check` passed cleanly in 0.28s with zero warnings or errors.

## Plan Adherence

Went exactly according to plan. Both edits were surgical (Edit tool, not file rewrites). The `compare.rs` module was pre-created by another agent; this agent only wired the CLI surface into `cli.rs` and `main.rs`.
