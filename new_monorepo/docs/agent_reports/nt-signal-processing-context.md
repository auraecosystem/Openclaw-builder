# Agent Report: NT Signal Processing Indicators

## Task

Port and create 4 new NautilusTrader signal processing indicators plus their PyO3 Python bindings,
following the repository's established `Indicator` trait pattern.

## Files Created

### Core Indicators (4 files)

1. `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/permutation_entropy.rs`
   - Permutation Entropy via Lehmer code ordinal-pattern hashing
   - Ring buffer of `period` close prices; recomputes on each update
   - Shannon entropy normalized by `ln(order!)` → value in [0, 1]
   - Constant series correctly produces 0.0 (all identical patterns)

2. `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/bayesian_changepoint.rs`
   - Bayesian Online Changepoint Detection (BOCPD) with NIG conjugate prior
   - Fully stateful (run-length distribution accumulates across bars)
   - Uses price returns (`close/prev_close - 1`) as observations
   - Includes Lanczos `ln_gamma` and Student-t predictive likelihood helpers
   - Run-length hypotheses pruned to `period` to bound memory

3. `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/kalman_trend_filter.rs`
   - Constant-velocity Kalman filter with state `[level, trend]`
   - Row-major 2×2 covariance stored as `[f64; 4]` — no heap allocation
   - Bootstraps level from first observation to avoid cold-start tracking error
   - Exposes `level`, `trend`, and `value` (= level) as public fields

4. `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/signal_processing/hurst_exponent.rs`
   - Hurst Exponent via Detrended Fluctuation Analysis (DFA)
   - Geometrically spaced box sizes (×1.5 per step) for efficient log-log regression
   - Requires minimum 16 samples; clamped to [0, 1]
   - Includes `box_rms`, `least_squares_slope`, `clamp_01` as private helpers

### PyO3 Bindings (4 files)

5. `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/permutation_entropy.rs`
   - Signature: `(period, order=None, delay=None)`
   - Exposes `order` and `delay` getters

6. `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/bayesian_changepoint.rs`
   - Signature: `(period, hazard_lambda=None, prior_mu=None, prior_kappa=None, prior_alpha=None, prior_beta=None)`
   - Exposes `hazard_lambda` getter

7. `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/kalman_trend_filter.rs`
   - Signature: `(period, process_noise=None, measurement_noise=None)`
   - Exposes extra `level` and `trend` getters as specified

8. `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/python/signal_processing/hurst_exponent.rs`
   - Signature: `(period,)`

### Module Registration

- **`signal_processing/mod.rs`**: Added `bayesian_changepoint`, `hurst_exponent`, `kalman_trend_filter`
  (the file already existed with `permutation_entropy` and others; added only the missing modules)
- **`python/signal_processing/mod.rs`**: Added `bayesian_changepoint`, `hurst_exponent`, `kalman_trend_filter`
  (already had `permutation_entropy` declared; `hidden_markov_regime` was also missing and added)
- **`python/mod.rs`** and **`lib.rs`**: Already had `signal_processing` declared and all 4 classes
  registered — no changes needed

## Adaptation from Plan

The directories and `mod.rs` files already existed (pre-populated from a previous agent's pass).
The `permutation_entropy.rs` I wrote was accepted without conflict. The `python/mod.rs` already
registered all 4 new classes by path. The only material adaptation was editing rather than
creating the mod files.

## Test Results

```
cargo test -p nautilus-indicators signal_processing
test result: ok. 53 passed; 0 failed; 0 ignored
```

All 53 signal_processing tests pass, including the 23 tests covering the 4 new indicators.

## Key Design Decisions

- **BOCPD memory bound**: Run-length vector pruned to `period` entries rather than growing
  unboundedly — matches the "ring buffer of period close prices" spirit for stateful indicators.
- **Kalman covariance**: Stored as `[f64; 4]` (no `ndarray` dependency) to keep stack-allocated,
  `unsafe`-free code consistent with repo constraints.
- **DFA box sizing**: Geometric ×1.5 step avoids an O(n) box-size loop while producing enough
  points for a robust log-log regression even at `period=32`.
- **Student-t PDF**: Computed in log space, then exponentiated, to prevent underflow at large
  `nu` values during BOCPD updates.
