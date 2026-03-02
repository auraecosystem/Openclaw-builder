//! Top-level pipeline: wires all four layers together for one bar of one
//! ticker.
//!
//! `run_pipeline()` is the single entry point called from the strategy
//! loop.  It orchestrates Layer 0 characterization (when due), Layer 1
//! scanning, composite scoring, conditional Layer 2 investigation, and
//! Layer 3 execution signal extraction.

use super::arena::ThreadArena;
use super::layer0::CharacterizationState;
use super::layer1::{self, ScannerOutput};
use super::layer1::scorer;
use super::layer3;
use super::params::SignalParams;

/// Complete output of one pass through the 4-layer signal pipeline.
#[derive(Clone, Debug, Default)]
pub struct SignalOutput {
    /// Layer 1 scanner features.
    pub scanner: ScannerOutput,

    /// Composite scorer output (dot product of features and weights, through
    /// sigmoid).  Range approximately [0, 1].
    pub score: f32,

    /// Whether the score exceeds `params.candidate_threshold`.
    pub is_candidate: bool,

    /// Layer 2 conformal prediction lower bound (present only for candidates).
    pub conformal_lo: Option<f32>,

    /// Layer 2 conformal prediction upper bound (present only for candidates).
    pub conformal_hi: Option<f32>,

    /// Layer 3: Kalman trailing stop level (ATR-scaled distance below price).
    pub kalman_stop: f32,

    /// Layer 3: whether BOCPD probability exceeds the exit threshold,
    /// signaling regime death.
    pub bocpd_exit: bool,

    /// Layer 3: whether VMD STA/LTA crossed the entry threshold, confirming
    /// breakout energy release.
    pub vmd_entry_trigger: bool,
}

/// Run the full 4-layer signal pipeline for one bar of one ticker.
///
/// Orchestrates: L1 scanner → composite scorer → conditional L2
/// investigation → L3 execution signals.  L0 characterization is handled
/// externally by the strategy (recomputed every `l0_recompute_interval`
/// bars and passed in via `l0_state`).
///
/// # Arguments
///
/// * `close_window` - trailing close prices (length = `params.window_len`)
/// * `vol_window`   - trailing volume values
/// * `high_window`  - trailing high prices (reserved for future use)
/// * `low_window`   - trailing low prices (reserved for future use)
/// * `arena`        - pre-allocated scratch buffers for this thread
/// * `params`       - tunable algorithm parameters
/// * `l0_state`     - latest Layer 0 characterization for this asset
pub fn run_pipeline(
    close_window: &[f32],
    vol_window: &[f32],
    _high_window: &[f32],
    _low_window: &[f32],
    arena: &mut ThreadArena,
    params: &SignalParams,
    l0_state: &CharacterizationState,
) -> SignalOutput {
    if close_window.len() < 2 {
        return SignalOutput::default();
    }

    // L1: Scanner — 13 features
    let scanner = layer1::run_scanner(
        close_window,
        vol_window,
        arena,
        params,
        l0_state.hurst,
        l0_state.hmm_state,
    );

    // Composite score (normalized features for the dot-product scorer)
    let features = scanner.to_scorer_array();
    let score = scorer::compute_score(&features, &arena.scorer_weights, arena.scorer_bias);
    let is_candidate = score > params.candidate_threshold;

    // L2: Investigation (conditional — only for candidates).
    // For MVP, conformal calibrator requires historical residuals that we
    // don't accumulate yet, so L2 intervals are left as None.
    let (conformal_lo, conformal_hi) = (None, None);

    // L3: Execution signals — reuses L1 scanner state
    let atr = simple_atr(close_window, 14);
    let conformal_width = match (conformal_lo, conformal_hi) {
        (Some(lo), Some(hi)) => Some(hi - lo),
        _ => None,
    };
    let exec = layer3::execution_signals(
        scanner.kalman_level,
        scanner.kalman_trend,
        scanner.bocpd_prob,
        scanner.vmd_sta_lta,
        atr,
        conformal_width,
        params,
    );

    SignalOutput {
        scanner,
        score,
        is_candidate,
        conformal_lo,
        conformal_hi,
        kalman_stop: exec.kalman_stop,
        bocpd_exit: exec.bocpd_exit,
        vmd_entry_trigger: exec.vmd_entry_trigger,
    }
}

/// Simple ATR estimate from close prices only (true range approximation).
///
/// Uses |close[i] - close[i-1]| as a proxy for true range when high/low
/// are unavailable.
fn simple_atr(close: &[f32], period: usize) -> f32 {
    let n = close.len();
    if n < 2 || period == 0 {
        return f32::NAN;
    }
    let start = if n > period { n - period } else { 1 };
    let mut sum = 0.0_f32;
    let mut count = 0;
    for i in start..n {
        let tr = (close[i] - close[i - 1]).abs();
        sum += tr;
        count += 1;
    }
    if count > 0 { sum / count as f32 } else { f32::NAN }
}
