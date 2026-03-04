# Agent Report: Evolution Genome + Fitness Modules

## Task

Create two new Rust files for the algotrader engine's evolution module:
1. `src/evolution/genome.rs` -- genome encoding/decoding between flat `Vec<f64>` and `Params`/`SignalParams`
2. `src/evolution/fitness.rs` -- fitness evaluation wrapping `run_single()` with scalar metric extraction

## Files Created

- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/evolution/genome.rs` (517 lines)
- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/evolution/fitness.rs` (257 lines)

## Implementation Details

### genome.rs

- `GeneBound` struct: name, min, max, is_integer, is_boolean metadata per gene.
- `GenomeSpec` struct: ordered `Vec<GeneBound>` defining a full genome layout.
- Three constructors: `params_spec(crypto)` (14 genes with stock/crypto range variants), `signal_params_spec()` (28 scalar fields + 13 scorer weights = 41 genes), `combined_spec(crypto)` (concatenation).
- `encode()` / `decode()` use explicit match statements on gene names for compile-time safety. Signal params fields fall through to a dedicated `encode_signal_field` / `decode_signal_field` match.
- Scorer weight names use a static `[&str; 13]` array (`SCORER_WEIGHT_NAMES`) so the gene names are `&'static str` without allocation.
- `clamp()` enforces bounds, rounds integers, thresholds booleans. `random()` generates uniform samples within bounds then clamps.
- 7 unit tests: round-trip params, round-trip combined, clamp bounds, crypto vs stock ranges, random within bounds, scorer weights round-trip, boolean gene threshold.

### fitness.rs

- `FitnessMetric` enum: Sharpe, Sortino, Return, ProfitFactor, Cagr, Growth, Composite.
- `from_str()` parses CLI strings; unknown falls back to Composite.
- `evaluate()` runs `crate::run_single()`, serializes report to `serde_json::Value`, extracts scalar.
- `extract_fitness()` is a direct port of Python `evolve.py`: trades < 10 returns -999.0, linear confidence ramp 0.3 at 10 trades to 1.0 at 50+, metric-specific scoring, composite formula with drawdown penalty.
- 6 unit tests: low trade count, composite matches Python values, confidence ramp, growth metric, from_str parsing, zero init cash, high drawdown kills composite.

## Constraints Followed

- No `mod.rs` created, no other files modified.
- All imports use `crate::` paths.
- `SCANNER_FEATURE_COUNT` (13) imported from `crate::signals::SCANNER_FEATURE_COUNT`.
- `rand::Rng` trait used for random genome generation.
- Types match existing crate: `Params` fields are `f32`/`u32`/`bool`, `SignalParams` fields are `f32`/`usize`.

## Outcome

Went according to plan. Both files are self-contained, compile-time type-safe via explicit match arms, and include comprehensive unit tests covering round-trips, boundary conditions, and parity with the Python evolve.py fitness formula.
