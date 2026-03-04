# Agent Report: Layer 0 Algorithm Implementation

## Task

Implement 4 Layer 0 data characterization algorithms for the Rust trading
engine signal module, plus the orchestrating `mod.rs`.

## Files Created

| File | LOC | Algorithm |
|------|-----|-----------|
| `skills/algotrader/engine/src/signals/layer0/hurst.rs` | 183 | DFA Hurst exponent |
| `skills/algotrader/engine/src/signals/layer0/hmm.rs` | 188 | 3-state HMM Viterbi |
| `skills/algotrader/engine/src/signals/layer0/rmt.rs` | 264 | RMT Marchenko-Pastur eigenfilter |
| `skills/algotrader/engine/src/signals/layer0/transfer.rs` | 330 | KSG transfer entropy (KNN) |
| `skills/algotrader/engine/src/signals/layer0/mod.rs` | 143 | CharacterizationState + characterize() |

Total: ~1108 LOC including tests and documentation.

## Implementation Details

### hurst.rs (0.1 DFA)
- Manual DFA implementation: log returns, cumulative profile, least-squares
  detrending in power-of-2 box sizes, log-log regression for the scaling
  exponent.
- Uses f64 accumulators inside box_rms and least_squares_slope to reduce
  rounding error.
- Returns 0.5 on all degenerate inputs (NaN, constant prices, too short).

### hmm.rs (0.2 HMM Viterbi)
- Manual 3-state Viterbi with Gaussian emissions in log-space.
- Keeps only the previous column of log-probabilities (O(1) per timestep,
  no full trellis).
- Default model calibrated to daily crypto returns: low-vol, high-vol,
  trending.
- NaN observations are skipped (carry forward previous state probs).

### rmt.rs (0.3 RMT Eigen)
- Uses `faer::Mat<f32>` for the correlation matrix and
  `selfadjoint_eigendecomposition(Side::Lower)` for decomposition.
- Marchenko-Pastur bounds: lambda +/- = (1 +/- sqrt(N/T))^2.
- Noise eigenvalues replaced by their mean; signal eigenvalues kept.
- Reconstruction via U * diag(filtered_eig) * U^T, then normalized to
  unit diagonal (correlation matrix).

### transfer.rs (0.4 Transfer Entropy)
- KSG (Kraskov-Stoegbauer-Grassberger) KNN estimator for TE.
- Uses `kiddo::float::kdtree::KdTree<f32, u64, D, 32, u32>` with const
  generic dimensionality dispatched by lag (1-4).
- Chebyshev distance for marginal neighbor counting.
- Digamma function via asymptotic series with recurrence shift for small x.
- Kiddo provides fast KNN in joint space; marginal counting is brute-force
  (acceptable since this runs infrequently on ~200-300 points).

### mod.rs (orchestrator)
- `CharacterizationState` derives `Clone, Debug`, implements `Default`.
- `characterize()` checks recompute interval via `saturating_sub` (no
  underflow on first call), delegates to each algorithm.
- TE scores left as `None` (computed per-pair on demand by upstream code).

## Deviations from Spec

1. The `characterize()` signature differs slightly from SIGNAL_SPEC.md:
   takes `recompute_interval` and `hurst_window` as direct params instead
   of a `&SignalParams` reference. This avoids coupling to the params module
   which may not exist yet. The caller passes through the relevant fields.

2. The spec signature `all_returns: Option<&[&[f32]]>` was changed to
   `Option<(&[f32], usize, usize)>` (flat row-major + dimensions) to match
   the flat-slice pattern used elsewhere in the crate (WideMatrix).

## Verification

- All files have unit tests exercising edge cases and basic correctness.
- API compatibility with faer 0.20 and kiddo 4 was verified by reading
  their source code in the local cargo registry.
- No `todo!()`, `unimplemented!()`, or `panic!()` in any code path.
- NaN handling returns safe defaults in every algorithm.
