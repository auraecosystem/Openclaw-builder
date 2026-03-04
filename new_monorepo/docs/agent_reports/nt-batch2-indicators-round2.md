# Agent Report: NT Batch 2 Signal Processing Indicators (Round 2)

## Task

Implement 5 NautilusTrader signal processing indicators and their PyO3 bindings per
`/tmp/nt-batch2-context.md` and the spec docs in
`/Users/ad/work/ai/openclaw/docs/reference/precursor-detection/`.

## Files Created

### Rust Indicator Implementations

- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/quickest_change_ar.rs`
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/first_passage_time.rs`
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/survival_model.rs`
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/scan_statistic.rs`
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/conformal_anomaly.rs`

### PyO3 Binding Files

- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/quickest_change_ar.rs`
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/first_passage_time.rs`
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/survival_model.rs`
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/scan_statistic.rs`
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/conformal_anomaly.rs`

### Module Registration (modified)

- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/mod.rs`
- `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/mod.rs`

## Algorithm Implementations

### QuickestChangeAR
- Yule-Walker AR(p) fitting via inline Levinson-Durbin recursion (zero external deps)
- Two-sided CUSUM on residuals with configurable threshold h and allowance k
- AR coefficients re-fit every `refit_interval` observations
- Defaults: period=100, ref_period=200, p=4, h=5.0, k=0.5, two_sided=true, refit_interval=100
- Extra outputs: `alarm: bool`, `residual: f64`

### FirstPassageTime
- Inverse Gaussian CDF: P(T_a ≤ t) = Φ((μt−a)/(σ√t)) + exp(2μa/σ²)·Φ(−(μt+a)/(σ√t))
- Normal CDF via Abramowitz & Stegun 26.2.17 rational approximation (max error < 7.5e-8), inline
- Drift/volatility from rolling log-returns
- Defaults: period=100, threshold_up=2.0, threshold_down=2.0, symmetric=true
- Extra outputs: `hazard: f64`, `expected_time: f64`, `elapsed: usize`

### SurvivalModel
- Nelson-Aalen cumulative hazard: H(t) = Σ d_i/n_i, S(t) = exp(-H(t))
- Ring buffer of inter-event times; events triggered at `event_threshold × σ` moves
- Censoring at `censoring_window` bars
- Defaults: period=500, event_threshold=2.0, censoring_window=50
- Extra outputs: `cumulative_hazard: f64`, `hazard_rate: f64`, `median_survival: f64`

### ScanStatistic
- Gaussian LLR scan over all (start, window_size) pairs using prefix sums for O(1) per window
- LLR(s,w) = (n/2) · ln(σ₀²/σ_z²); σ_z² is pooled in/out-window variance under H₁
- Monte Carlo p-values omitted (too expensive per bar per spec)
- Defaults: period=500, w_min=5, w_max=period/4, baseline_period=200
- Extra outputs: `best_window_start: usize`, `best_window_size: usize`

### ConformalAnomaly
- k-NN nonconformity via partial sort (1-D, exact, no external deps)
- Conformal p-value: (|{i: α_i ≥ α_t}| + 1) / (|D_cal| + 1)
- Power martingale accumulated in log-space to avoid overflow
- Defaults: period=200, k=5, epsilon=0.05, mart_power=0.5, mart_threshold=20.0, use_martingale=true
- Extra outputs: `martingale: f64`, `anomaly: bool`

## Test Results

All 5 new indicators pass their full test suites (40+ tests total):

```
signal_processing::quickest_change_ar::tests::test_alarm_triggered_on_large_shift ... ok
signal_processing::first_passage_time::tests::test_normal_cdf_symmetry ... ok
signal_processing::survival_model::tests::test_event_recorded_on_large_move ... ok
signal_processing::scan_statistic::tests::test_spike_increases_llr ... ok
signal_processing::conformal_anomaly::tests::test_anomaly_on_outlier ... ok
... (all 40 new tests pass)
```

`cargo check -p nautilus-indicators` produces zero warnings after minor fixes.

## Adaptations

1. The mod.rs files had grown beyond what the context snapshot showed (10 additional modules from prior batches). New modules were inserted in alphabetical order.

2. Four pre-existing modules had a missing `use crate::stubs::bar_ethusdt_binance_minute_bid;` import (left behind by a prior agent). This was blocking the entire test binary from compiling. Fixed as a prerequisite.

3. `sqrt_t` was removed from `inverse_gaussian_pdf` after the compiler flagged it unused (the PDF formula uses `t * t * t` directly, not `sqrt_t`).

4. One pre-existing failure in `approximate_entropy` (unrelated to this batch) remains: `test_value_is_finite_after_inputs` panics on `a.value >= 0.0`. Not in scope.
