//! Tunable parameters for the signal processing pipeline.
//!
//! Every field has a serde default so `SignalParams::default()` produces a
//! valid, ready-to-use configuration.  Field groupings follow the layer and
//! algorithm numbering from `signal-stack.md`.

use serde::{Deserialize, Serialize};

use super::feature::SCANNER_FEATURE_COUNT;

/// All tunable knobs for the 4-layer signal pipeline.
///
/// Organized by algorithm. Defaults are chosen as sensible starting points
/// from the algorithm reference; tune via grid search or evolutionary
/// optimization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalParams {
    // -- Global ----------------------------------------------------------

    /// General lookback window length (bars) for column extraction.
    #[serde(default = "default_window_len")]
    pub window_len: usize,

    /// Composite score threshold above which the LLM agent is woken.
    #[serde(default = "default_candidate_threshold")]
    pub candidate_threshold: f32,

    /// How often (in bars) Layer 0 characterization is recomputed.
    #[serde(default = "default_l0_recompute_interval")]
    pub l0_recompute_interval: usize,

    // -- 1.1 Permutation Entropy -----------------------------------------

    /// Embedding dimension for PE ordinal ranking.
    #[serde(default = "default_pe_order")]
    pub pe_order: usize,

    /// Time delay between elements in PE embedding.
    #[serde(default = "default_pe_delay")]
    pub pe_delay: usize,

    /// Rolling window length for PE computation.
    #[serde(default = "default_pe_window")]
    pub pe_window: usize,

    // -- 1.2 BOCPD -------------------------------------------------------

    /// Hazard rate parameter (expected run length before changepoint).
    #[serde(default = "default_bocpd_lambda")]
    pub bocpd_lambda: f32,

    // -- 1.3 Kalman Filter ------------------------------------------------

    /// Process noise covariance for the Kalman state transition.
    #[serde(default = "default_kalman_q")]
    pub kalman_q: f32,

    /// Measurement noise covariance for the Kalman observation.
    #[serde(default = "default_kalman_r")]
    pub kalman_r: f32,

    // -- 1.4 VMD ----------------------------------------------------------

    /// Number of VMD decomposition modes (trend, swing, noise).
    #[serde(default = "default_vmd_k_modes")]
    pub vmd_k_modes: usize,

    /// VMD bandwidth constraint (higher = narrower modes).
    #[serde(default = "default_vmd_alpha")]
    pub vmd_alpha: f32,

    /// Short-term average window for STA/LTA on VMD swing mode.
    #[serde(default = "default_vmd_sta_window")]
    pub vmd_sta_window: usize,

    /// Long-term average window for STA/LTA on VMD swing mode.
    #[serde(default = "default_vmd_lta_window")]
    pub vmd_lta_window: usize,

    // -- 1.5 KNN Anomaly -------------------------------------------------

    /// Number of nearest neighbors for the anomaly score.
    #[serde(default = "default_knn_k")]
    pub knn_k: usize,

    // -- 1.6 Scattering Transform ----------------------------------------

    /// Number of octaves (dyadic scales) for wavelet scattering.
    #[serde(default = "default_scatter_j")]
    pub scatter_j: usize,

    // -- 1.8 STOMP --------------------------------------------------------

    /// Subsequence length for the matrix profile computation.
    #[serde(default = "default_stomp_m")]
    pub stomp_m: usize,

    // -- 1.9 Template Matching --------------------------------------------

    /// Length of the z-normalized template window.
    #[serde(default = "default_template_len")]
    pub template_len: usize,

    // -- 1.10 SWT Denoising -----------------------------------------------

    /// Decomposition level for the stationary wavelet transform.
    #[serde(default = "default_swt_level")]
    pub swt_level: usize,

    // -- 0.1 Hurst --------------------------------------------------------

    /// Lookback window for the rolling DFA Hurst exponent.
    #[serde(default = "default_hurst_window")]
    pub hurst_window: usize,

    // -- 0.2 HMM ----------------------------------------------------------

    /// Number of hidden states in the Gaussian HMM.
    #[serde(default = "default_hmm_n_states")]
    pub hmm_n_states: usize,

    // -- 0.3 RMT ----------------------------------------------------------

    /// Window length for the RMT correlation eigendecomposition.
    #[serde(default = "default_rmt_window")]
    pub rmt_window: usize,

    // -- 2.1 Conformal Prediction -----------------------------------------

    /// Target coverage probability for conformal prediction intervals.
    #[serde(default = "default_conformal_coverage")]
    pub conformal_coverage: f32,

    /// Calibration window length for conformal nonconformity scores.
    #[serde(default = "default_conformal_window")]
    pub conformal_window: usize,

    // -- 1.7 VPIN ---------------------------------------------------------

    /// Number of equal-volume buckets for VPIN estimation.
    #[serde(default = "default_vpin_n_buckets")]
    pub vpin_n_buckets: usize,

    // -- Scorer -----------------------------------------------------------

    /// Per-feature weights for the composite dot-product scorer.
    #[serde(default = "default_scorer_weights")]
    pub scorer_weights: [f32; SCANNER_FEATURE_COUNT],

    /// Additive bias applied after the weighted sum, before sigmoid.
    #[serde(default = "default_scorer_bias")]
    pub scorer_bias: f32,

    // -- Layer 3 Execution ------------------------------------------------

    /// Multiplier on ATR for the Kalman trailing stop distance.
    #[serde(default = "default_kalman_stop_atr_mult")]
    pub kalman_stop_atr_mult: f32,

    /// BOCPD probability threshold that triggers an immediate exit.
    #[serde(default = "default_bocpd_exit_threshold")]
    pub bocpd_exit_threshold: f32,

    /// VMD STA/LTA threshold that confirms a breakout entry.
    #[serde(default = "default_vmd_entry_threshold")]
    pub vmd_entry_threshold: f32,
}

// ---------------------------------------------------------------------------
// Serde default functions
// ---------------------------------------------------------------------------

fn default_window_len() -> usize { 200 }
fn default_candidate_threshold() -> f32 { 0.5 }
fn default_l0_recompute_interval() -> usize { 100 }

fn default_pe_order() -> usize { 5 }
fn default_pe_delay() -> usize { 1 }
fn default_pe_window() -> usize { 30 }

fn default_bocpd_lambda() -> f32 { 100.0 }

fn default_kalman_q() -> f32 { 0.001 }
fn default_kalman_r() -> f32 { 1.0 }

fn default_vmd_k_modes() -> usize { 3 }
fn default_vmd_alpha() -> f32 { 2000.0 }
fn default_vmd_sta_window() -> usize { 10 }
fn default_vmd_lta_window() -> usize { 50 }

fn default_knn_k() -> usize { 5 }
fn default_scatter_j() -> usize { 6 }
fn default_stomp_m() -> usize { 30 }
fn default_template_len() -> usize { 50 }
fn default_swt_level() -> usize { 3 }

fn default_hurst_window() -> usize { 500 }
fn default_hmm_n_states() -> usize { 3 }
fn default_rmt_window() -> usize { 500 }

fn default_conformal_coverage() -> f32 { 0.90 }
fn default_conformal_window() -> usize { 200 }
fn default_vpin_n_buckets() -> usize { 50 }

fn default_scorer_weights() -> [f32; SCANNER_FEATURE_COUNT] {
    [1.0 / SCANNER_FEATURE_COUNT as f32; SCANNER_FEATURE_COUNT]
}
fn default_scorer_bias() -> f32 { 0.0 }

fn default_kalman_stop_atr_mult() -> f32 { 2.0 }
fn default_bocpd_exit_threshold() -> f32 { 0.7 }
fn default_vmd_entry_threshold() -> f32 { 2.0 }

// ---------------------------------------------------------------------------
// Manual Default impl — mirrors serde defaults exactly.
// ---------------------------------------------------------------------------

impl Default for SignalParams {
    fn default() -> Self {
        Self {
            window_len: default_window_len(),
            candidate_threshold: default_candidate_threshold(),
            l0_recompute_interval: default_l0_recompute_interval(),

            pe_order: default_pe_order(),
            pe_delay: default_pe_delay(),
            pe_window: default_pe_window(),

            bocpd_lambda: default_bocpd_lambda(),

            kalman_q: default_kalman_q(),
            kalman_r: default_kalman_r(),

            vmd_k_modes: default_vmd_k_modes(),
            vmd_alpha: default_vmd_alpha(),
            vmd_sta_window: default_vmd_sta_window(),
            vmd_lta_window: default_vmd_lta_window(),

            knn_k: default_knn_k(),
            scatter_j: default_scatter_j(),
            stomp_m: default_stomp_m(),
            template_len: default_template_len(),
            swt_level: default_swt_level(),

            hurst_window: default_hurst_window(),
            hmm_n_states: default_hmm_n_states(),
            rmt_window: default_rmt_window(),

            conformal_coverage: default_conformal_coverage(),
            conformal_window: default_conformal_window(),
            vpin_n_buckets: default_vpin_n_buckets(),

            scorer_weights: default_scorer_weights(),
            scorer_bias: default_scorer_bias(),

            kalman_stop_atr_mult: default_kalman_stop_atr_mult(),
            bocpd_exit_threshold: default_bocpd_exit_threshold(),
            vmd_entry_threshold: default_vmd_entry_threshold(),
        }
    }
}
