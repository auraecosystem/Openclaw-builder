# Engine Architecture

Post-refactoring reference. Three phases completed: config extraction, signal flattening, workspace split.

## Workspace Crates

```
engine-types   (serde only, leaf)
     ^                ^
engine-data (polars)  engine-signals (rustfft, faer, kiddo)
     ^                ^
     +----------------+
              ^
algotrader-engine (rayon, rand, clap)
```

- **engine-types** -- `Params`, `Trade`, `Direction`, `SignalParams`, `SignalSet`, matrix types, 6 config structs (`ExecutionConfig`, `StrategyConfig`, `FitnessConfig`, `GaConfig`, `CmaEsConfig`, `AnalysisConfig`). All configs use `#[serde(default)]` for zero-config backward compat.
- **engine-data** -- `DataStore`, `Indicator` enum, parquet loader. Polars isolated here.
- **engine-signals** -- 18 algorithms in flat `algorithms/` directory, `pipeline.rs` orchestrator, `scorer.rs`, `execution_signals.rs`, `arena.rs`. No layer hierarchy.
- **algotrader-engine** -- execution simulator, 4 strategies, analysis/reporting, CMA-ES + GA evolution, CLI, TCP server.

## File Layout

```
engine/
  Cargo.toml                 workspace root, resolver = "2"
  crates/
    types/src/               lib, types, signal_params, feature, matrix,
                             execution_config, strategy_config, fitness_config,
                             ga_config, cmaes_config, analysis_config
    data/src/                lib, indicator, loader, store
    signals/src/             lib, pipeline, scorer, execution_signals, arena,
                             algorithms/ (18 modules)
  src/                       root crate
    lib.rs, main.rs, cli.rs
    execution/               position, simulator, fills, exits
    strategy/                breakout, ep, parabolic, signal_breakout
    analysis/                metrics, report
    evolution/               cmaes, fitness, ga, genome
    server/                  tcp
    walk_forward.rs
```

## Key Design Decisions

- **SignalParams uses algorithm-purpose naming** (`kalman_trend_amplifier`, not `l3_trend_amplifier`) so field names survive any future reorganization.
- **RawAlgorithmOutputs + normalize_for_scorer()** replaced `ScannerOutput`. All algorithms write raw values; normalization happens once before scoring.
- **Single pipeline.rs dispatch** replaced 4 per-layer dispatch functions.
- **Incremental rebuild**: touching a signal recompiles `engine-signals` + root (~1s). Polars never recompiles unless `engine-data` changes.
- **Root lib.rs re-exports**: `pub use engine_types as types;` etc., so downstream code uses `algotrader_engine::types::Params`.

## Build

```bash
cd engine && cargo build --release    # ~2.5 min cold, ~1s incremental
cargo test                            # 248 tests
```
