# Wave 3: Integration, CMA-ES, Strategy Configs

## Summary

Wired the engine-pipeline crate into the root crate's strategy factory and CMA-ES
evolution system. Created example dynamic strategy JSON configs.

## Changes Made

### 1. `engine/src/strategy/mod.rs` -- DynamicSetupAdapter + Factory Fallback

Added a dynamic fallback branch to `create_setups()`. When an unknown strategy name
is requested, it searches for `strategies/{name}.json` relative to CARGO_MANIFEST_DIR
(compile-time) and the current working directory (runtime fallback).

Key additions:
- `DynamicSetupAdapter` struct that wraps `engine_pipeline::DynamicSetup` and
  implements the root crate's `Setup` trait
- `convert_exit_rule()` function mapping `DynamicExitRule` to `ExitRule`
- `load_dynamic_setup()` and `dynamic_strategy_paths()` helper functions
- All hardcoded strategies continue to work identically (no behavioral change)

### 2. `engine/src/evolution/genome.rs` -- `GenomeSpec::from_pipeline_params()`

Added a method that builds a `GenomeSpec` from a pipeline config's `params` HashMap.
Each param with min/max bounds becomes a gene. Bool params with `evolvable: true`
become continuous [0,1] genes thresholded at 0.5. Params are sorted by name for
deterministic gene ordering. Uses `Box::leak` for param names (acceptable since
genome specs are built once at startup).

### 3. `engine/strategies/` -- Dynamic Strategy JSON Configs

Created three new files:
- `blocks.json` -- shared block library with stock_universe chain
- `breakout_quick.json` -- dynamic equivalent of BreakoutQuick
- `ep_dynamic.json` -- dynamic equivalent of EpisodicPivot

## Verification

- `cargo check` -- workspace compiles cleanly
- `cargo test --workspace` -- 440 tests pass (0 failures)
- `cargo test -p engine-pipeline` -- 110 pipeline tests pass
- All existing hardcoded strategies unchanged

## Files Modified

- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/strategy/mod.rs`
- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/evolution/genome.rs`

## Files Created

- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/strategies/blocks.json`
- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/strategies/breakout_quick.json`
- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/strategies/ep_dynamic.json`
