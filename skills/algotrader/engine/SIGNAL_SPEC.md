# Signal Module — Rust Implementation Contracts

Algorithm details: `../signal-stack.md`. This doc covers Rust types, signatures, and conventions only.

## Crate Dependencies

| Crate | Used by |
|-------|---------|
| `rustfft 6` | VMD (1.4), scattering (1.6), STOMP (1.8), template (1.9), SWT (1.10) |
| `faer 0.20` | RMT eigendecomposition (0.3) |
| `kiddo 4` | Transfer entropy (0.4), KNN anomaly (1.5), Renyi TE (2.2) |

## File Layout

```
src/signals/
├── mod.rs              # pub use re-exports
├── arena.rs            # ThreadArena (pre-allocated scratch per rayon thread)
├── params.rs           # SignalParams (~50 tunable fields, all serde defaults)
├── feature.rs          # FeatureVec: [f32; SCANNER_FEATURE_COUNT]
├── pipeline.rs         # run_pipeline() → SignalOutput
├── layer0/
│   ├── mod.rs          # CharacterizationState, characterize()
│   ├── hurst.rs        # 0.1 DFA
│   ├── hmm.rs          # 0.2 HMM Viterbi (manual 3×3)
│   ├── rmt.rs          # 0.3 RMT eigen (faer)
│   └── transfer.rs     # 0.4 Transfer Entropy (kiddo)
├── layer1/
│   ├── mod.rs          # ScannerOutput, run_scanner(), SCANNER_FEATURE_COUNT
│   ├── perm_entropy.rs # 1.1
│   ├── bocpd.rs        # 1.2 (manual Bayesian online changepoint)
│   ├── kalman.rs       # 1.3 (manual 2×2)
│   ├── vmd.rs          # 1.4 (rustfft)
│   ├── knn_anomaly.rs  # 1.5 (kiddo)
│   ├── scattering.rs   # 1.6 (rustfft)
│   ├── vpin.rs         # 1.7 (manual)
│   ├── stomp.rs        # 1.8 (rustfft)
│   ├── template.rs     # 1.9 (rustfft)
│   ├── swt.rs          # 1.10 (rustfft)
│   └── scorer.rs       # dot(features, weights) + sigmoid
├── layer2/
│   ├── mod.rs          # investigate()
│   ├── conformal.rs    # 2.1
│   ├── renyi.rs        # 2.2 (kiddo)
│   └── kronos.rs       # 2.3 stub (pyo3, cold path)
└── layer3/
    └── mod.rs          # execution_signals(): reuses L1 state
```

## Key Types

### SCANNER_FEATURE_COUNT = 13

```rust
// layer1/mod.rs
pub const SCANNER_FEATURE_COUNT: usize = 13;

pub struct ScannerOutput {
    pub perm_entropy: f32,   // 1.1
    pub bocpd_prob: f32,     // 1.2
    pub kalman_level: f32,   // 1.3
    pub kalman_trend: f32,   // 1.3
    pub vmd_sta_lta: f32,    // 1.4
    pub knn_anomaly: f32,    // 1.5
    pub scatter_class: f32,  // 1.6
    pub vpin: f32,           // 1.7
    pub mp_novelty: f32,     // 1.8
    pub template_corr: f32,  // 1.9
    pub swt_denoised: f32,   // 1.10
    pub hurst: f32,          // 0.1 pass-through
    pub hmm_state: f32,      // 0.2 pass-through
}

impl ScannerOutput {
    pub fn to_array(&self) -> [f32; SCANNER_FEATURE_COUNT];
}
```

### SignalOutput

```rust
// pipeline.rs
pub struct SignalOutput {
    pub scanner: ScannerOutput,
    pub score: f32,               // composite scorer output
    pub is_candidate: bool,       // score > threshold
    // Layer 2 (None if not candidate)
    pub conformal_lo: Option<f32>,
    pub conformal_hi: Option<f32>,
    // Layer 3 execution signals
    pub kalman_stop: f32,         // reuses 1.3
    pub bocpd_exit: bool,         // reuses 1.2
    pub vmd_entry_trigger: bool,  // reuses 1.4
}
```

### CharacterizationState

```rust
// layer0/mod.rs
pub struct CharacterizationState {
    pub hurst: f32,
    pub hmm_state: u8,           // 0, 1, or 2
    pub cleaned_corr: Option<Vec<f32>>, // N×N flattened, from RMT
    pub te_scores: Option<Vec<f32>>,    // N, from transfer entropy
    pub last_updated: usize,     // bar index of last recompute
}

pub fn characterize(
    close: &[f32],       // full close column for one ticker
    all_returns: Option<&[&[f32]]>, // N tickers' returns (for RMT/TE)
    bar_idx: usize,
    params: &SignalParams,
    prev: &CharacterizationState,
) -> CharacterizationState;
```

### ThreadArena

```rust
// arena.rs
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
    // VMD modes: K × fft_len
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
// params.rs — all fields have serde defaults
pub struct SignalParams {
    // Global
    pub window_len: usize,          // default 200
    pub candidate_threshold: f32,   // default 0.5
    pub l0_recompute_interval: usize, // default 100

    // Per-algorithm (examples, not exhaustive)
    pub pe_order: usize,            // default 5
    pub pe_delay: usize,            // default 1
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

    // Scorer weights
    pub scorer_weights: [f32; SCANNER_FEATURE_COUNT], // default all 1.0/13
    pub scorer_bias: f32,           // default 0.0

    // Evolution helpers
    // pub fn to_genome(&self) -> Vec<f64>;
    // pub fn from_genome(g: &[f64]) -> Self;
}
```

## Function Signatures (per-algorithm)

Every Layer 1 algorithm follows the same pattern:

```rust
pub fn compute_<name>(
    window: &[f32],          // pre-extracted close (or denoised) window
    arena: &mut ThreadArena, // scratch buffers
    params: &SignalParams,
) -> f32;                    // single normalized output
```

Layer 0 functions take broader context (multiple tickers, longer history).
Layer 2 functions take `ScannerOutput` + additional context.
Layer 3 functions reuse Layer 1 state from `ThreadArena`.

## Integration Points

### New Setup: `SignalBreakout` in `strategy/signal_breakout.rs`
Implements existing `Setup` trait. Uses `breakout_filter()` for universe, runs `run_pipeline()` per bar.

### New ExitRule variants in `execution/exits.rs`
```rust
SignalBocpdExit { threshold: f32 },
SignalKalmanStop { atr_mult: f32 },
```

### Params extension in `types.rs`
```rust
pub signal_params: Option<SignalParams>,  // serde default None, backward compatible
```

### Factory extension in `strategy/mod.rs`
```rust
"signal_breakout" => vec![Box::new(SignalBreakout)],
```

## Conventions

- `f32` everywhere (not f64) — matches existing WideMatrix
- Zero heap alloc in hot loop — use ThreadArena scratch buffers
- Each algorithm in its own file, single `pub fn compute_*` entry point
- NaN propagation: if input contains NaN, return NaN (or 0.0 for scores)
- All Layer 1 algorithms are independent — no cross-dependencies
