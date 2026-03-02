//! Pre-allocated scratch buffers for zero-allocation hot-loop execution.
//!
//! Each rayon thread owns one `ThreadArena`.  Buffers are sized once from
//! `SignalParams` and reused across all tickers.  Per-ticker stateful fields
//! (Kalman state, BOCPD run lengths, PE ordinal counts) are zeroed between
//! tickers via `reset_ticker()`.

use rustfft::num_complex::Complex32;

use super::feature::SCANNER_FEATURE_COUNT;
use super::layer1::bocpd::BocpdState;
use super::params::SignalParams;

/// Scratch memory for one rayon worker thread.
///
/// All vectors are allocated in `new()` and never resized during the backtest.
/// This keeps the per-bar, per-ticker inner loop allocation-free.
pub struct ThreadArena {
    // -- FFT scratch (shared across VMD, STOMP, SWT, scattering, template) --
    pub fft_scratch: Vec<Complex32>,

    // -- General-purpose scratch (3 reusable f32 buffers) -------------------
    pub scratch: [Vec<f32>; 3],

    // -- Pre-extracted column windows ---------------------------------------
    pub close_w: Vec<f32>,
    pub vol_w: Vec<f32>,
    pub high_w: Vec<f32>,
    pub low_w: Vec<f32>,

    // -- VMD modes: K x fft_len complex buffers -----------------------------
    pub vmd_modes: Vec<Vec<Complex32>>,

    // -- STOMP distance profile ---------------------------------------------
    pub stomp_dp: Vec<f32>,

    // -- Stateful across bars, reset per ticker -----------------------------

    /// Kalman state vector [level, trend].
    pub kalman_x: [f32; 2],

    /// Kalman covariance matrix (2x2 stored flat, row-major).
    pub kalman_p: [f32; 4],

    /// BOCPD online state (run-length distribution + sufficient statistics).
    pub bocpd_state: BocpdState,

    // -- PE ordinal counts (pe_order! possible permutations) ----------------
    pub perm_counts: Vec<u32>,

    // -- Scorer weights (copied once from params) ---------------------------
    pub scorer_weights: [f32; SCANNER_FEATURE_COUNT],
    pub scorer_bias: f32,
}

/// Factorial helper for small n (only used for PE order, typically 5! = 120).
fn factorial(n: usize) -> usize {
    (1..=n).product()
}

impl ThreadArena {
    /// Allocate all scratch buffers from the given parameters.
    ///
    /// Call once per rayon thread at backtest start.  The FFT scratch buffer
    /// is sized to the next power of two above `window_len` to accommodate
    /// the real-to-complex transforms used by VMD, STOMP, and SWT.
    pub fn new(params: &SignalParams) -> Self {
        let w = params.window_len;
        // FFT buffers need power-of-two length for radix-2 algorithms.
        let fft_len = w.next_power_of_two();
        let n_perms = factorial(params.pe_order);

        Self {
            fft_scratch: vec![Complex32::new(0.0, 0.0); fft_len],

            scratch: [
                vec![0.0_f32; w],
                vec![0.0_f32; w],
                vec![0.0_f32; w],
            ],

            close_w: vec![0.0; w],
            vol_w: vec![0.0; w],
            high_w: vec![0.0; w],
            low_w: vec![0.0; w],

            vmd_modes: (0..params.vmd_k_modes)
                .map(|_| vec![Complex32::new(0.0, 0.0); fft_len])
                .collect(),

            stomp_dp: vec![0.0; w.saturating_sub(params.stomp_m).max(1)],

            kalman_x: [0.0; 2],
            kalman_p: [1.0, 0.0, 0.0, 1.0], // identity covariance
            bocpd_state: BocpdState::new(w),
            perm_counts: vec![0; n_perms],

            scorer_weights: params.scorer_weights,
            scorer_bias: params.scorer_bias,
        }
    }

    /// Zero per-ticker stateful fields between tickers.
    ///
    /// Scratch buffers and column windows are overwritten each bar so they
    /// do not need clearing.  Only the carried-forward Kalman state, BOCPD
    /// run-length distribution, and PE ordinal counts must be reset.
    pub fn reset_ticker(&mut self) {
        self.kalman_x = [0.0; 2];
        self.kalman_p = [1.0, 0.0, 0.0, 1.0];

        self.bocpd_state.reset();

        for c in self.perm_counts.iter_mut() {
            *c = 0;
        }
    }
}
