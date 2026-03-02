# Signal Module -- Rust Implementation Contracts

Algorithm details: `../signal-stack.md`. This doc covers Rust types, signatures, and conventions only.

## Crate Dependencies

| Crate | Used by |
|-------|---------|
| `rustfft 6` | VMD (1.4), scattering (1.6), STOMP (1.8), template (1.9), SWT (1.10) |
| `faer 0.20` | RMT eigendecomposition (0.3) |
| `kiddo 4` | Transfer entropy (0.4), KNN anomaly (1.5), Renyi TE (2.2) |

## File Layout

```
crates/signals/src/
├── lib.rs              # pub use re-exports
├── arena.rs            # ThreadArena (pre-allocated scratch per rayon thread)
├── pipeline.rs         # run_pipeline() -> SignalOutput (consolidated orchestrator)
├── scorer.rs           # dot(features, weights) + sigmoid
├── execution_signals.rs # adaptive stops, exits, sizing (was layer3/mod.rs)
└── algorithms/
    ├── mod.rs          # re-exports
    ├── bocpd.rs        # 1.2 Bayesian changepoint
    ├── conformal.rs    # 2.1 conformal prediction
    ├── hmm.rs          # 0.2 HMM Viterbi
    ├── hurst.rs        # 0.1 DFA Hurst exponent
    ├── kalman.rs       # 1.3 Kalman filter (2x2)
    ├── knn_anomaly.rs  # 1.5 KNN anomaly
    ├── kronos.rs       # 2.3 foundation model (stub)
    ├── perm_entropy.rs # 1.1 permutation entropy
    ├── renyi.rs        # 2.2 Renyi transfer entropy
    ├── rmt.rs          # 0.3 RMT eigendecomposition
    ├── scattering.rs   # 1.6 wavelet scattering
    ├── stomp.rs        # 1.8 matrix profile
    ├── swt.rs          # 1.10 wavelet denoising
    ├── template.rs     # 1.9 LIGO template matching
    ├── transfer.rs     # 0.4 transfer entropy
    ├── vmd.rs          # 1.4 VMD decomposition
    └── vpin.rs         # 1.7 VPIN order flow
```

## Key Types

### SCANNER_FEATURE_COUNT = 13

```rust
// crates/types/src/feature.rs
pub const SCANNER_FEATURE_COUNT: usize = 13;
pub type FeatureVec = [f32; SCANNER_FEATURE_COUNT];
```

`RawAlgorithmOutputs` (in `pipeline.rs`) stores raw algorithm values. The old `ScannerOutput` struct and `to_array()` are replaced by `normalize_for_scorer()` which maps price-scale features to scorer-friendly ranges. A backward-compatible type alias `ScannerOutput = RawAlgorithmOutputs` exists during migration.

```rust
// crates/signals/src/pipeline.rs
pub struct RawAlgorithmOutputs {
    pub perm_entropy: f32,   // 1.1
    pub bocpd_prob: f32,     // 1.2
    pub kalman_level: f32,   // 1.3 (raw price units)
    pub kalman_trend: f32,   // 1.3 (raw price units/bar)
    pub vmd_sta_lta: f32,    // 1.4 (raw, typically 0-10)
    pub knn_anomaly: f32,    // 1.5
    pub scatter_class: f32,  // 1.6
    pub vpin: f32,           // 1.7
    pub mp_novelty: f32,     // 1.8
    pub template_corr: f32,  // 1.9
    pub swt_denoised: f32,   // 1.10 (raw price units)
    pub hurst: f32,          // 0.1 pass-through
    pub hmm_state: f32,      // 0.2 pass-through
    pub last_close: f32,     // used by normalize_for_scorer()
    // Investigation outputs (previously Layer 2)
    pub conformal_lo: f32,
    pub conformal_hi: f32,
    pub renyi_te_standard: f32,
    pub renyi_te_tail: f32,
    pub kronos_direction: f32,
    pub kronos_confidence: f32,
}

impl RawAlgorithmOutputs {
    /// Normalize features to ~[0,1] or ~[-1,1] for the composite scorer.
    /// Price-scale features (kalman, swt) are divided by last_close.
    /// Raw fields are preserved for execution signals.
    pub fn normalize_for_scorer(&self, params: &SignalParams) -> [f32; SCANNER_FEATURE_COUNT];

    /// Backward-compatible alias for normalize_for_scorer().
    pub fn to_scorer_array(&self, params: &SignalParams) -> [f32; SCANNER_FEATURE_COUNT];
}
```

### SignalOutput

```rust
// crates/signals/src/pipeline.rs
pub struct SignalOutput {
    pub scanner: RawAlgorithmOutputs,
    pub score: f32,               // composite scorer output
    pub is_candidate: bool,       // score > threshold
    // Investigation (None if not candidate)
    pub conformal_lo: Option<f32>,
    pub conformal_hi: Option<f32>,
    // Execution signals
    pub kalman_stop: f32,         // reuses 1.3
    pub bocpd_exit: bool,         // reuses 1.2
    pub vmd_entry_trigger: bool,  // reuses 1.4
}
```

### CharacterizationState

```rust
// crates/signals/src/pipeline.rs
pub struct CharacterizationState {
    pub hurst: f32,
    pub hmm_state: u8,           // 0, 1, or 2
    pub cleaned_corr: Option<Vec<f32>>, // N*N flattened, from RMT
    pub te_scores: Option<Vec<f32>>,    // N, from transfer entropy
    pub last_updated: usize,     // bar index of last recompute
}

pub fn characterize(
    close: &[f32],
    returns: &[f32],
    all_returns: Option<(&[f32], usize, usize)>,
    bar_idx: usize,
    params: &SignalParams,
    prev: &CharacterizationState,
) -> CharacterizationState;
```

### ThreadArena

```rust
// crates/signals/src/arena.rs
pub struct ThreadArena {
    // FFT (shared across VMD, STOMP, SWT, scattering, template)
    pub fft_scratch: Vec<rustfft::num_complex::Complex32>,
    // General scratch (3 reusable buffers)
    pub scratch: [Vec<f32>; 3],
    // Pre-extracted column windows
    pub close_w: Vec<f32>,
    pub vol_w: Vec<f32>,
    pub high_w: Vec<f32>,
    pub low_w: Vec<f32>,
    // VMD modes: K * fft_len
    pub vmd_modes: Vec<Vec<rustfft::num_complex::Complex32>>,
    // STOMP distance profile
    pub stomp_dp: Vec<f32>,
    // Stateful across bars (reset per ticker)
    pub kalman_x: [f32; 2],
    pub kalman_p: [f32; 4],
    pub bocpd_run_lengths: Vec<f32>,
    // PE ordinal counts (5! = 120)
    pub perm_counts: Vec<u32>,
    // Scorer weights (copied from params once)
    pub scorer_weights: [f32; SCANNER_FEATURE_COUNT],
    pub scorer_bias: f32,
}

impl ThreadArena {
    pub fn new(params: &SignalParams) -> Self;
    pub fn reset_ticker(&mut self);  // clear per-ticker state
}
```

### SignalParams

```rust
// crates/types/src/signal_params.rs -- all fields have serde defaults
pub struct SignalParams {
    // Global
    pub window_len: usize,          // default 200
    pub candidate_threshold: f32,   // default 0.5
    pub l0_recompute_interval: usize, // default 100

    // Per-algorithm (examples, not exhaustive)
    pub pe_order: usize,            // default 5
    pub pe_delay: usize,            // default 1
    pub pe_window: usize,           // default 30
    pub bocpd_lambda: f32,          // default 100.0
    pub kalman_q: f32,              // default 0.001
    pub kalman_r: f32,              // default 1.0
    pub vmd_k_modes: usize,         // default 3
    pub vmd_alpha: f32,             // default 2000.0
    pub vmd_sta_window: usize,      // default 10
    pub vmd_lta_window: usize,      // default 50
    pub knn_k: usize,               // default 5
    pub scatter_j: usize,           // default 6
    pub stomp_m: usize,             // default 30
    pub template_len: usize,        // default 50
    pub swt_level: usize,           // default 3
    pub hurst_window: usize,        // default 500
    pub hmm_n_states: usize,        // default 3
    pub rmt_window: usize,          // default 500
    pub conformal_coverage: f32,    // default 0.90
    pub conformal_window: usize,    // default 200
    pub vpin_n_buckets: usize,      // default 50

    // Scorer weights
    pub scorer_weights: [f32; SCANNER_FEATURE_COUNT], // default all 1.0/13
    pub scorer_bias: f32,           // default 0.0

    // Execution signals
    pub kalman_stop_atr_mult: f32,  // default 2.0
    pub bocpd_exit_threshold: f32,  // default 0.7
    pub vmd_entry_threshold: f32,   // default 2.0

    // Algorithm internals (promoted from hardcoded constants)
    pub atr_period: usize,          // default 14
    pub vmd_n_iter: usize,          // default 15
    pub vmd_tau: f32,               // default 0.0
    pub bocpd_kappa0: f32,          // default 1.0
    pub bocpd_mu0: f32,             // default 0.0
    pub bocpd_alpha0: f32,          // default 1.0
    pub bocpd_beta0: f32,           // default 1.0
    pub hmm_transition: [[f32; 3]; 3],  // default [[0.90,0.05,0.05],[0.10,0.80,0.10],[0.05,0.10,0.85]]
    pub hmm_emission_mean: [f32; 3],    // default [0.0, 0.0, 0.002]
    pub hmm_emission_var: [f32; 3],     // default [0.0001, 0.001, 0.0004]
    pub hmm_init_probs: [f32; 3],       // default [0.4, 0.3, 0.3]

    // Normalization constants (scorer input mapping)
    pub kalman_vel_clamp: f32,      // default 1.0
    pub swt_norm_clamp: f32,        // default 0.1
    pub swt_norm_scale: f32,        // default 10.0
    pub vmd_norm_divisor: f32,      // default 5.0

    // Execution signal constants (adaptive stop/sizing)
    pub kalman_trend_amplifier: f32,   // default 100.0
    pub kalman_trend_up_cap: f32,      // default 1.0
    pub kalman_trend_down_cap: f32,    // default 0.5
    pub conformal_size_floor: f32,     // default 0.1
    pub conformal_size_ceil: f32,      // default 3.0

    // Evolution helpers
    // pub fn to_genome(&self) -> Vec<f64>;
    // pub fn from_genome(g: &[f64]) -> Self;
}
```

## Function Signatures (per-algorithm)

Every algorithm module in `algorithms/` follows the same pattern:

```rust
pub fn compute_<name>(
    window: &[f32],          // pre-extracted close (or denoised) window
    arena: &mut ThreadArena, // scratch buffers
    params: &SignalParams,
) -> f32;                    // single normalized output
```

The pipeline orchestrator in `pipeline.rs` calls each algorithm and feeds the collected outputs through `normalize_for_scorer()` into the scorer. Algorithms do not know about layers or each other -- they receive `&[f32]` slices and return scalars.

Characterization algorithms (hurst, hmm, rmt, transfer) take broader context (multiple tickers, longer history) and are called on a slower cadence (`l0_recompute_interval` bars).

Investigation algorithms (conformal, renyi, kronos) take `RawAlgorithmOutputs` plus additional context and only run for candidates that passed the scorer threshold.

## Integration Points

### New Setup: `SignalBreakout` in `strategy/signal_breakout.rs`
Implements existing `Setup` trait. Uses `breakout_filter()` for universe, runs `run_pipeline()` per bar.

### New ExitRule variants in `execution/exits.rs`
```rust
SignalBocpdExit { threshold: f32 },
SignalKalmanStop { atr_mult: f32 },
```

### Params extension in `crates/types/src/types.rs`
```rust
pub signal_params: Option<crate::signal_params::SignalParams>,  // serde default None, backward compatible
```

### Factory extension in `strategy/mod.rs`
```rust
"signal_breakout" => vec![Box::new(SignalBreakout)],
```

## Conventions

- `f32` everywhere (not f64) -- matches existing WideMatrix
- Zero heap alloc in hot loop -- use ThreadArena scratch buffers
- Each algorithm in its own file, single `pub fn compute_*` entry point
- NaN propagation: if input contains NaN, return NaN (or 0.0 for scores)
- All algorithms are independent -- no cross-dependencies between algorithm modules
- `engine-signals` depends only on `engine-types`, NOT on `engine-data` (signals take `&[f32]` slices, never touch polars)
- Config structs live in `engine-types` -- signals reference them via `engine_types::SignalParams`
