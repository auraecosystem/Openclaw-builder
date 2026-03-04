# Wave 2A -- Plan Compilation + Filter Fusion

## Status: Complete

All work completed as specified. Both files implemented, all tests passing.

## Files Modified

- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/crates/pipeline/src/plan.rs` -- LogicalStep, PhysicalStep, PhysicalPlan, compile_plan()
- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/crates/pipeline/src/fuse.rs` -- FusedCondition, FusedFilter, fuse_steps()

## What Was Built

### plan.rs

- `LogicalStep` struct: validated pipeline step with resolved block name, config HashMap, optional when-condition, optional input ref, step ID. Built from `StepDef` via `from_step_def()`.
- `PhysicalStep` enum: `Single { block_name, config, step_id, input_id, when }` or `Fused(FusedFilter)`.
- `PhysicalPlan` struct: `filter_steps: Vec<PhysicalStep>`, `signal_step: Option<PhysicalStep>`, plus `stop` and `exits` configs carried through for DynamicSetup.
- `compile_plan()`: walks pipeline, builds LogicalSteps, identifies signal step (first block with output_type == Signals), splits into filter phase + signal phase, applies fusion to filter phase.

### fuse.rs

- `FusedCondition` struct: `{ indicator: Indicator, op: CmpOp, threshold: f32 }` with inline `check()` method.
- `FusedFilter` struct: `{ conditions: Vec<FusedCondition>, step_id: String }` with `execute()` method that pre-fetches indicator matrices, does a single pass with short-circuit evaluation per cell.
- `fuse_steps()`: scans consecutive LogicalSteps, accumulates fusable runs (no when, no explicit input, fusable_spec present, threshold resolvable from config), emits FusedFilter for runs and Single for breaks.
- `try_build_condition()`: resolves the threshold f32 from the step's config map using the FusableSpec's threshold_key.

## Tests Written

### plan.rs (9 tests)
- All-fusable steps produce single FusedFilter
- Non-fusable step breaks a run into Fused + Single + Fused
- When-conditional prevents fusion
- Signal step identified correctly
- Empty pipeline
- Stop/exits carried through
- LogicalStep requires ID
- LogicalStep requires block type
- Explicit input prevents fusion

### fuse.rs (17 tests)
- FusedCondition::check for all 4 CmpOp variants (Gt, Ge, Lt, Le)
- FusedFilter::execute correctness with 2 conditions
- FusedFilter::execute with 3 conditions
- FusedFilter::execute respects input mask
- FusedFilter::execute NaN fails condition
- FusedFilter::execute empty input produces empty output
- FusedFilter::execute partial range
- fuse_steps with 0 steps
- fuse_steps with 1 fusable step
- fuse_steps with 5 consecutive fusable steps
- Non-fusable breaking a run
- When-conditional preventing fusion
- Explicit input preventing fusion
- Missing threshold preventing fusion (falls back to Single)

## Verification

- `cargo check -p engine-pipeline` passes with zero warnings in owned files
- `cargo test -p engine-pipeline` passes all 100 tests (including pre-existing tests from Waves 0 and 1)
- No changes made to files outside ownership scope

## Design Decisions

1. A single fusable step still gets wrapped in a FusedFilter (1-condition filter). This avoids special-casing and the FusedFilter path handles it efficiently.
2. When a fusable block's threshold cannot be resolved from config (missing key or non-numeric value), the step falls back to a Single dispatch rather than erroring. This is defensive -- the block's own execute() will produce a better error message.
3. Composite step IDs for multi-step fused filters use the format `fused_s0_s1_s2` for debuggability. Single-step fused filters keep their original step ID.