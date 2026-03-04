# Agent Report: Workspace Split

## Task

Split the monolithic `algotrader-engine` Rust crate into a Cargo workspace with 3 sub-crates:
- `engine-types` (leaf, serde only)
- `engine-data` (polars isolated)
- `engine-signals` (rustfft, faer, kiddo)

## Outcome

Successful. The workspace compiles cleanly and all 31 passing tests continue to pass. The 2 pre-existing sharpe test failures remain unchanged.

## What Was Done

### Directory Structure Created

```
engine/
  Cargo.toml              (workspace root)
  crates/
    types/Cargo.toml       (engine-types: serde only)
    types/src/lib.rs
    types/src/types.rs, matrix.rs, feature.rs, signal_params.rs,
           execution_config.rs, strategy_config.rs, fitness_config.rs,
           ga_config.rs, cmaes_config.rs, analysis_config.rs
    data/Cargo.toml        (engine-data: polars, rayon, strum, anyhow, chrono)
    data/src/lib.rs
    data/src/indicator.rs, loader.rs, store.rs
    signals/Cargo.toml     (engine-signals: rustfft, faer, kiddo)
    signals/src/lib.rs
    signals/src/algorithms/ (17 algorithm files + mod.rs)
    signals/src/arena.rs, pipeline.rs, scorer.rs, execution_signals.rs
  src/                     (root crate: algotrader-engine)
    lib.rs, main.rs, cli.rs, walk_forward.rs
    execution/, strategy/, analysis/, evolution/, server/, bin/
  tests/                   (unchanged, all imports work via re-exports)
```

### Key Decisions and Adaptations

1. **Re-export strategy**: Root `lib.rs` uses `pub use engine_types as types;`, `pub use engine_data as data;`, `pub use engine_signals as signals;` so all existing test imports (`algotrader_engine::types::X`, `algotrader_engine::signals::X`, etc.) continue to resolve without changes to test files.

2. **Backward-compatible module paths**: Added `pub use engine_types::signal_params as params;` and `pub use engine_types::feature;` to `engine-signals/src/lib.rs` so paths like `signals::params::SignalParams` still work.

3. **Matrix visibility**: Changed `pub(crate) data` fields on `WideMatrix` and `WideMask` to `pub` in the types crate, plus `pub(crate) fn data_mut` to `pub fn data_mut`, since cross-crate access requires full pub visibility.

4. **Root crate kept `polars` and `faer`**: The root crate still needs `polars` for `bin/build_cache.rs` (standalone binary) and `faer` for `evolution/cmaes.rs` (stays in root crate). Added `chrono` to engine-data since `loader.rs` uses it for date resolution.

5. **Config module re-exports**: Each root module that had a `config.rs` (execution, strategy, analysis, evolution) now re-exports the corresponding engine-types module. For example, `execution/mod.rs` has `pub use engine_types::execution_config as config;` so `execution::config::ExecutionConfig` still resolves.

### Files Deleted from Root src/

- `src/types.rs` (moved to crates/types/)
- `src/data/` (entire directory, moved to crates/data/)
- `src/signals/` (entire directory, moved to crates/signals/)
- `src/execution/config.rs` (moved to crates/types/)
- `src/strategy/config.rs` (moved to crates/types/)
- `src/analysis/config.rs` (moved to crates/types/)
- `src/evolution/fitness_config.rs` (moved to crates/types/)
- `src/evolution/ga_config.rs` (moved to crates/types/)
- `src/evolution/cmaes_config.rs` (moved to crates/types/)

### Verification

- `cargo check --workspace` passes cleanly
- `cargo test --workspace` shows 31 passed, 2 failed (pre-existing sharpe test failures)
