# Agent Report: NT Batch 2 Signal Processing Indicators

## Task

Implement 4 signal processing indicators for NautilusTrader per `/tmp/nt-batch2-context.md`:

1. `ApproximateEntropy` — template-matching regularity measure with self-inclusion
2. `SampleEntropy` — bias-corrected regularity measure (excludes self-matches)
3. `RecurrenceQuantification` — RQA via time-delay embedding (DET, LAM, ENTR, RR, L_mean, TT)
4. `VisibilityGraph` — NVG/HVG graph topology metrics (mean_degree, clustering, assortativity)

## Files Created

### Rust Indicator Files

- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/approximate_entropy.rs` (281 lines)
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/sample_entropy.rs` (287 lines)
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/recurrence_quantification.rs` (495 lines)
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/visibility_graph.rs` (363 lines)

### PyO3 Binding Files

- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/approximate_entropy.rs` (110 lines)
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/sample_entropy.rs` (110 lines)
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/recurrence_quantification.rs` (177 lines)
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/visibility_graph.rs` (111 lines)

### Updated Module Registration Files

- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/mod.rs` — added 3 new pub mod entries
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/mod.rs` — added 3 new pub mod entries
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/mod.rs` — registered 4 new classes with `m.add_class::<...>()`

## Implementation Notes

### ApproximateEntropy

- Parameters: `period` (200), `m` (2), `r` (0.2), `r_absolute` (None) — all `Option<T>`
- `r_absolute` takes precedence over relative `r * std` when set
- Self-matches included via j ranging over all indices including i
- Chebyshev distance for template comparison
- Returns 0.0 for constant series (all templates identical)

### SampleEntropy

- Same interface as ApproximateEntropy
- j == i pairs are skipped (no self-matches)
- Counts B (m-length) and A (m+1-length) matches globally, not per-template
- Returns `f64::INFINITY` when A = 0 (no longer matches)
- Constant series yields infinity (as expected per spec)

### RecurrenceQuantification

- 8 parameters: `period`, `m`, `tau`, `epsilon`, `epsilon_absolute`, `l_min`, `v_min`, `theiler`
- Memory-efficient: no N'xN' matrix stored — diagonal and vertical run lengths tracked via 1-D vectors
- Theiler window correction applied by subtracting main diagonal and side-diagonal histogram contributions
- Outputs: `value` (= DET), `rr`, `det`, `lam`, `entr`, `l_mean`, `tt` — all `pub f64`
- Euclidean distance for phase-space vectors

### VisibilityGraph

- Parameters: `period` (100), `use_hvg` (false) — all `Option<T>`
- NVG: O(N²) pairwise check with geometric visibility condition
- HVG: O(N) walk with early termination when a bar blocks the view
- Adjacency list representation (`Vec<Vec<usize>>`) used for all metric computations
- Outputs: `value` (= `mean_degree`), `mean_degree`, `clustering`, `assortativity` — all `pub f64`

## Build Status

`cargo check -p nautilus-indicators` passes cleanly (0 errors, 0 warnings from new files).

The test binary fails to compile due to pre-existing errors in OTHER agent files
(`scan_statistic.rs`, `survival_model.rs`, `quickest_change_ar.rs`, `robust_dispersion.rs`,
`cusum_control_chart.rs`, `ewma_control_chart.rs`) that are missing
`use crate::stubs::bar_ethusdt_binance_minute_bid;` in their test modules.
These files are outside this agent's assigned scope.

All 4 new indicator files correctly use `use crate::stubs::bar_ethusdt_binance_minute_bid;`
in their test modules and produce no compilation errors.

## Spec Compliance

All parameters are `Option<T>` with proper defaults via `.unwrap_or()`. All extra outputs
beyond `value` are exposed as `pub f64` fields and as `#[getter]` methods in PyO3 bindings.
Tests cover: name, display, not_initialized, initialized_after_period, value correctness,
reset, and handle_bar.
