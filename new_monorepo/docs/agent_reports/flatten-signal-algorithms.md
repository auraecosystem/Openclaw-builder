# Agent Report: Flatten Signal Algorithm Module Structure

## Task

Remove layer numbering from the Rust algotrader engine signal module and make algorithms composable by flattening `layer0/`, `layer1/`, `layer2/`, `layer3/` into a single `algorithms/` directory with orchestration logic consolidated in `pipeline.rs`.

## Outcome

Completed successfully. All 272 tests pass (2 pre-existing sharpe failures excluded as instructed). Zero compilation warnings.

## Changes Made

### Structural moves (17 algorithm files)

- `src/signals/layer0/{hurst,hmm,rmt,transfer}.rs` -> `src/signals/algorithms/`
- `src/signals/layer1/{bocpd,kalman,knn_anomaly,perm_entropy,scattering,stomp,swt,template,vmd,vpin}.rs` -> `src/signals/algorithms/`
- `src/signals/layer2/{conformal,renyi,kronos}.rs` -> `src/signals/algorithms/`
- `src/signals/layer1/scorer.rs` -> `src/signals/scorer.rs`
- `src/signals/layer3/mod.rs` -> `src/signals/execution_signals.rs`
- Deleted: `layer0/mod.rs`, `layer1/mod.rs`, `layer2/mod.rs`, and all four layer directories

### New files

- `src/signals/algorithms/mod.rs` -- `pub mod` declarations for all 17 algorithms

### Consolidated pipeline.rs

Merged logic from 4 files into a single orchestrator:

- `CharacterizationState` struct + `characterize()` function (from `layer0/mod.rs`)
- `RawAlgorithmOutputs` struct with `normalize_for_scorer()` + `run_scanner()` (from `layer1/mod.rs`)
- `InvestigationResult` struct + `investigate()` function (from `layer2/mod.rs`)
- `SignalOutput` struct + `run_pipeline()` entry point (original `pipeline.rs`)
- Added `ScannerOutput` type alias and `to_scorer_array()` backward-compat method
- Preserved the intentional `norm_kalman_vel` duplicate (both slots get same value)

### Import path updates

| Old path | New path |
|---|---|
| `signals::layer0::hurst` | `signals::algorithms::hurst` |
| `signals::layer0::CharacterizationState` | `signals::pipeline::CharacterizationState` (or `signals::CharacterizationState` via re-export) |
| `signals::layer1::ScannerOutput` | `signals::pipeline::ScannerOutput` (type alias for `RawAlgorithmOutputs`) |
| `signals::layer1::scorer` | `signals::scorer` |
| `signals::layer1::bocpd` | `signals::algorithms::bocpd` |
| `signals::layer2::conformal` | `signals::algorithms::conformal` |
| `signals::layer3` | `signals::execution_signals` |

### Files updated for import changes

- `src/signals/mod.rs` -- module declarations and re-exports
- `src/signals/arena.rs` -- BocpdState import
- `src/strategy/signal_breakout.rs` -- characterize import + call site
- `tests/signal_layer0_test.rs` -- algorithm + type imports
- `tests/signal_layer1_test.rs` -- algorithm + scorer + type imports, added `..Default::default()` to struct literal
- `tests/signal_layer2_test.rs` -- conformal import
- `tests/signal_layer3_test.rs` -- execution_signals import + call sites

## Adaptation

The `RawAlgorithmOutputs` struct has 6 extra fields (investigation outputs) compared to the original `ScannerOutput`. This caused one test file (`signal_layer1_test.rs`) to fail when constructing a struct literal. Fixed by adding `..Default::default()` to the test's struct initializer.

No other deviations from the plan were necessary.
