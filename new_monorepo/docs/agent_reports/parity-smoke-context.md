# Agent Report: src/compare.rs

## Task

Create `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/compare.rs` — a comparison library for parity smoke testing between hardcoded Rust strategies and their dynamic JSON equivalents.

## Files Modified

- **Created**: `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/compare.rs`
- **Modified**: `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/lib.rs` (added `pub mod compare;`)

## Implementation Summary

### Public API

```rust
pub fn compare_strategies(store, params, hardcoded_name, dynamic_name) -> Result<CompareResult>
pub fn compare_all(store, params) -> Vec<CompareResult>
```

`compare_strategies` runs both strategies through the full filter → signals → simulate pipeline, measuring timing at each phase. It isolates signal differences by running both signal generators on the hardcoded filter mask.

`compare_all` runs three pairs: `("ep", "ep_dynamic")`, `("parabolic", "parabolic_dynamic")`, `("breakout", "breakout_quick")`. Failures are non-fatal (eprintln + skip).

### Key Design Decisions

1. **Only first setup used for multi-setup strategies**: `create_setups("breakout")` returns two setups (quick + runner). The spec requires comparing the first only, which matches `BreakoutQuick`.

2. **Signal isolation via hardcoded filter**: The `SignalComparison` section runs both setups' signal generators on the same (hardcoded) filter mask. This isolates whether signal logic differs independent of filter differences.

3. **Trade matching by (entry_row, ticker_col, direction) key**: Uses `HashSet` intersection/difference to count matched vs. exclusive trades without sorting. The `Trade` struct already carries `pnl` so no manual price arithmetic is needed.

4. **Total timing includes filter time**: `hardcoded_total_ms` is `signal_phase_elapsed + hardcoded_filter_ms` (not a separate wall-clock measurement). This avoids re-running pipelines just for timing.

5. **Jaccard edge case**: When both masks are entirely empty, Jaccard returns `1.0` (identical, both pass nothing) rather than 0/0 NaN.

6. **Mismatch collection capped at 10**: The `FilterComparison.mismatches` vec collects at most 10 `(row, col)` pairs to keep output readable in smoke test output.

## Compilation Result

```
Finished `dev` profile [optimized + debuginfo] target(s) in 0.96s
```

Zero warnings, zero errors.

## Adaptations

- The spec said to use `t.exit_price - t.entry_price` for PnL calculation, but `Trade` already has a `pnl: f64` field that is computed correctly (accounting for direction and shares). Used `t.pnl` directly, which is more accurate.
- The spec imported `WideMatrix` in the function signatures but it was not needed in the compare helpers (only `WideMask` and `SignalSet` are consumed there). The import was kept only where needed to avoid unused import warnings.
- `std::cell::Cell` was used briefly during `compare_masks` to track the `identical` flag inside a non-mutable closure pattern, then inlined to a plain mutable local instead for clarity.
