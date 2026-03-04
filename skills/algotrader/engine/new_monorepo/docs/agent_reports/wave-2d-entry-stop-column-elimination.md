# Agent Report: Wave 2d — Entry/Stop Block Column Elimination

## Task

Update the inner column loops in `entry.rs` and `stop.rs` to use `BlockContext::col_indices()` instead of iterating `0..nc`, enabling sparse column elimination when `alive_cols` is set on the context.

## Files Modified

- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/crates/pipeline/src/blocks/entry.rs`
- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/crates/pipeline/src/blocks/stop.rs`

## Transformation Applied

Each block's `execute()` method received two changes:

1. After `let nc = ctx.store.axes.n_cols;`, added:
   ```rust
   let all_cols: Vec<u32> = (0..nc as u32).collect();
   ```

2. Every `for col in 0..nc {` replaced with:
   ```rust
   for &col in ctx.col_indices(&all_cols) {
       let col = col as usize;
   ```

The `let col = col as usize;` rebinding preserves all downstream indexing without touching any logic inside the loops.

## Blocks Updated

### entry.rs (4 blocks)

| Block | Location |
|---|---|
| `BreakoutAbove::execute()` | line ~125 |
| `GapEntry::execute()` | line ~196 |
| `ScoreThreshold::execute()` | line ~263 |
| `ParabolicEntry::execute()` | line ~319 |

`fill_sma10_exits()` was left untouched — it is a standalone helper that does not receive a `BlockContext` and therefore cannot call `col_indices()`.

### stop.rs (3 blocks)

| Block | Location |
|---|---|
| `AtrCappedLow::execute()` | line ~61 |
| `FixedPct::execute()` | line ~112 |
| `GapDayLow::execute()` | line ~150 |

## Execution

All 7 edits applied cleanly with no surprises. No logic inside any loop was altered. Test code was not touched.
