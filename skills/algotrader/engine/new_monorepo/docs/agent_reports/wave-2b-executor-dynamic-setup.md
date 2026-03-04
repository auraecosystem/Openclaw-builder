# Wave 2B -- Executor + DynamicSetup

## Summary

Implemented the pipeline executor and DynamicSetup in `engine-pipeline` crate.
All 110 tests pass. Full workspace compiles clean with zero warnings.

## Files Modified

### `/Users/ad/work/ai/openclaw/skills/algotrader/engine/crates/pipeline/src/execute.rs`

Built from scratch (was a one-line stub). Contains:

- **Placeholder types**: `FusedFilter`, `FusedCondition`, `PhysicalStep`, `PhysicalPlan`, `PhysicalSignalStep` -- defined locally with TODO comments for replacement when Agent 2A's `plan.rs`/`fuse.rs` types are ready. These are structurally identical to what the plan describes.
- **`FusedFilter::execute()`**: Single-pass vectorized scan over indicator matrices with short-circuit per cell. Pre-fetches all indicator matrices before the loop.
- **`execute_filter_phase()`**: Seeds a Blackboard with an all-true mask, then iterates PhysicalSteps. Handles both Fused and Single step variants. Single steps check the `when` conditional gate and skip if false (passing through previous mask).
- **`execute_signal_phase()`**: Seeds blackboard with universe mask, dispatches to signal block, extracts SignalSet. Returns empty SignalSet if no signal step configured.
- **5 tests**: empty steps (all-true), single block filter, fused filter, when-false skip, empty signal phase.

### `/Users/ad/work/ai/openclaw/skills/algotrader/engine/crates/pipeline/src/lib.rs`

Expanded from module declarations to full DynamicSetup implementation. Contains:

- **`build_default_registry()`**: Registers all 31 blocks from all 7 block modules (universe, pattern, entry, stop, combiner, signal, cross_sectional). Lives in lib.rs to avoid registry.rs depending on every block module.
- **`parse_indicator_str()`**: Duplicated from `blocks/universe.rs` for lib-level use. Maps 26 indicator string names to `Indicator` enum variants.
- **`DynamicSetup` struct**: Holds name, direction, equity_fraction, fill_mode, exit_defs, stop_def, pipeline_config, and Arc<BlockRegistry>.
- **`DynamicSetup::from_json()`**: Deserializes StrategyPipeline, builds default registry, runs parse phase (assign_step_ids, interpolate_params), validates, constructs DynamicSetup.
- **`DynamicSetup::from_file()`**: Reads JSON file, delegates to from_json.
- **`DynamicSetup::compile_plan()`**: Simple compilation -- each StepDef becomes a Single PhysicalStep. Full fusion deferred to plan.rs/fuse.rs.
- **`DynamicExitRule` enum**: Local mirror of root crate's ExitRule (7 variants). Maps ExitDef configs to typed rules.
- **`map_exit_def()`** / **`map_exit_defs()`**: Maps all 7 exit types from JSON config to DynamicExitRule.
- **Public API methods**: `run_filter()`, `run_signals()`, `exit_rules()`, `name()`, `direction()`, `equity_fraction()`, `fill_mode_config()`.
- **12 tests**: from_json, direction mapping (long/short), fill_mode mapping, exit_rules for all 7 types, registry completeness check.

## Design Decisions

1. **Placeholder types in execute.rs**: Since Agent 2A owns plan.rs/fuse.rs, I defined the PhysicalPlan types locally in execute.rs. These match the plan's specification and are marked with TODO comments. When 2A's types become available, these can be replaced with re-exports.

2. **DynamicExitRule instead of root ExitRule**: engine-pipeline cannot depend on the root crate (circular dependency). Defined a local DynamicExitRule enum that the Wave 3 integration agent will map to the real ExitRule.

3. **parse_indicator_str duplication**: The universe.rs version is `fn` (crate-private). Rather than making it `pub` (not my file to edit), I duplicated the 26-line match in lib.rs. Both are simple exhaustive matches over the Indicator enum.

4. **build_default_registry in lib.rs**: The task said not to edit registry.rs. Placing the builder in lib.rs keeps registry.rs untouched while still wiring all 31 blocks.

## Verification

- `cargo check -p engine-pipeline`: zero warnings
- `cargo test -p engine-pipeline`: 110 passed, 0 failed
- `cargo check` (full workspace): clean