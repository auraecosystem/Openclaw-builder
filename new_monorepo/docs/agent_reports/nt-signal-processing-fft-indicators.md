# Agent Report: NT Signal Processing FFT Indicators

## Task

Port VMD + STA/LTA and Matrix Profile (STOMP) indicators into the NautilusTrader
indicators crate, plus wire up their module declarations and PyO3 bindings.

Six files were requested:

1. `signal_processing/variational_mode_decomposition.rs` — VMD core logic
2. `signal_processing/matrix_profile.rs` — STOMP/MASS core logic
3. `signal_processing/mod.rs` — module declarations
4. `python/signal_processing/mod.rs` — Python binding declarations
5. `python/signal_processing/variational_mode_decomposition.rs` — PyO3 bindings for VMD
6. `python/signal_processing/matrix_profile.rs` — PyO3 bindings for MatrixProfile

## What Was Found

- `Cargo.toml` already had `rustfft = { version = "6", optional = true }` and
  `signal-processing = ["rustfft"]` feature — no Cargo.toml edits needed.
- `signal_processing/` directory already contained 7 other indicator files
  (`bayesian_changepoint.rs`, `hidden_markov_regime.rs`, `hurst_exponent.rs`,
  `kalman_trend_filter.rs`, `permutation_entropy.rs`, `volume_clock.rs`,
  `wavelet_denoiser.rs`) but had no `mod.rs`.
- `python/signal_processing/` already had 5 binding files but no `mod.rs`.

## Adaptations

- The `signal_processing/mod.rs` and `python/signal_processing/mod.rs` were
  written to declare **all** existing modules (not only the spec-listed subset),
  preventing dead-module compile errors.
- `#![cfg(feature = "signal-processing")]` inner attribute is used at the top
  of each FFT file to gate the entire file, matching the spec instruction.
- `FftPlanner` is created inside the compute function (not stored on the struct)
  because `FftPlanner` does not implement `Debug`, which would prevent the struct
  from deriving `Debug` without a manual impl.
- `#[repr(C)]` + `#[derive(Debug)]` + `unsendable` pyclass attribute match the
  established NT indicator pattern.
- `rolling_mean_std` in `matrix_profile.rs` uses a naive O(n*m) loop rather than
  an online algorithm; this is intentional for correctness and simplicity given
  the window sizes typical in indicator usage.

## Files Created / Modified

All paths relative to `/Users/ad/work/ai/nautilus_trader/crates/indicators/src/`:

| File | Action |
|------|--------|
| `signal_processing/mod.rs` | Created |
| `signal_processing/variational_mode_decomposition.rs` | Created |
| `signal_processing/matrix_profile.rs` | Created |
| `python/signal_processing/mod.rs` | Created |
| `python/signal_processing/variational_mode_decomposition.rs` | Created |
| `python/signal_processing/matrix_profile.rs` | Created |

## Key Code Snippets

### VMD struct (signal_processing/variational_mode_decomposition.rs)

```rust
#[repr(C)]
#[derive(Debug)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_trader.core.nautilus_pyo3.indicators", unsendable)
)]
pub struct VariationalModeDecomposition {
    pub period: usize,
    pub k_modes: usize,
    pub alpha: f64,
    pub sta_window: usize,
    pub lta_window: usize,
    pub n_iter: usize,
    pub tau: f64,
    pub value: f64,
    pub count: usize,
    pub initialized: bool,
    has_inputs: bool,
    prices: Vec<f64>,
}
```

### MatrixProfile struct (signal_processing/matrix_profile.rs)

```rust
#[repr(C)]
#[derive(Debug)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_trader.core.nautilus_pyo3.indicators", unsendable)
)]
pub struct MatrixProfile {
    pub period: usize,
    pub subsequence_len: usize,
    pub value: f64,
    pub count: usize,
    pub initialized: bool,
    has_inputs: bool,
    prices: Vec<f64>,
}
```

## Status

All 6 files written. No compilation was run (no build environment invoked as per
constraints). The implementation follows the exact NT indicator pattern and the
VMD/STOMP algorithm source from the task spec.
