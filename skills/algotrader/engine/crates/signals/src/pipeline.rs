//! Signal pipeline: single orchestrator that wires all algorithms together
//! for one bar of one ticker.
//!
//! `run_pipeline()` is the single entry point called from the strategy loop.
//! It runs characterization (when due), scanning, composite scoring,
//! conditional investigation, and execution signal extraction.

use engine_types::{PatternParams, PatternScores, SCANNER_FEATURE_COUNT};
use engine_types::SignalParams;

use super::algorithms::{
    conformal, hmm, hurst, kalman, kronos, perm_entropy, renyi, rmt, stomp, swt, vmd, vpin,
};
use super::arena::ThreadArena;
use super::execution_signals;
use super::scorer;

// ---------------------------------------------------------------------------
// CharacterizationState (previously in layer0/mod.rs)
// ---------------------------------------------------------------------------

/// Snapshot of the current market characterization.
///
/// Carried between bar updates and only recomputed when
/// `bar_idx - last_updated >= l0_recompute_interval`.
#[derive(Clone, Debug)]
pub struct CharacterizationState {
    /// DFA Hurst exponent in [0, 1]. H > 0.55 = trending, H < 0.45 = mean-rev.
    pub hurst: f32,
    /// HMM regime state: 0 = low-vol, 1 = high-vol, 2 = trending.
    pub hmm_state: u8,
    /// RMT-filtered correlation matrix (N*N flattened row-major), if available.
    pub cleaned_corr: Option<Vec<f32>>,
    /// Transfer entropy scores (N values), if computed.
    pub te_scores: Option<Vec<f32>>,
    /// Bar index at which this state was last recomputed.
    pub last_updated: usize,
}

impl Default for CharacterizationState {
    fn default() -> Self {
        Self {
            hurst: 0.5,
            hmm_state: 0,
            cleaned_corr: None,
            te_scores: None,
            last_updated: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// RawAlgorithmOutputs (previously ScannerOutput in layer1/mod.rs)
// ---------------------------------------------------------------------------

/// Aggregated output of all algorithm computations for one bar of one ticker.
///
/// Fields store **raw** algorithm outputs so execution signals can consume them
/// at their natural scale (e.g. `kalman_level` in price units for the trailing
/// stop). The scorer receives normalized features via `normalize_for_scorer()`.
#[derive(Clone, Debug, Default)]
pub struct RawAlgorithmOutputs {
    /// Permutation entropy (0 = perfectly ordered, 1 = random).
    pub perm_entropy: f32,
    /// BOCPD changepoint probability at current bar.
    pub bocpd_prob: f32,
    /// Kalman-filtered price level (raw, in price units).
    pub kalman_level: f32,
    /// Kalman-filtered trend velocity (raw, price units/bar).
    pub kalman_trend: f32,
    /// VMD STA/LTA ratio on the swing mode (raw, typically 0-10).
    pub vmd_sta_lta: f32,
    /// KNN anomaly distance score.
    pub knn_anomaly: f32,
    /// Wavelet scattering classification score.
    pub scatter_class: f32,
    /// VPIN informed-trading estimate.
    pub vpin: f32,
    /// Matrix profile novelty (inverted nearest-motif distance).
    pub mp_novelty: f32,
    /// Template matching max cross-correlation.
    pub template_corr: f32,
    /// SWT denoised price output (raw, in price units).
    pub swt_denoised: f32,
    /// Hurst exponent pass-through from characterization.
    pub hurst: f32,
    /// HMM state pass-through from characterization (raw integer cast to f32).
    pub hmm_state: f32,
    /// Last close price -- used by `normalize_for_scorer()` for normalization.
    pub last_close: f32,

    // -- Investigation outputs (previously Layer 2) --

    /// Conformal prediction interval lower offset.
    pub conformal_lo: f32,
    /// Conformal prediction interval upper offset.
    pub conformal_hi: f32,
    /// Renyi transfer entropy at alpha=2 (standard regime coupling).
    pub renyi_te_standard: f32,
    /// Renyi transfer entropy at alpha=5 (tail-regime coupling).
    pub renyi_te_tail: f32,
    /// Kronos model predicted direction (-1 down, 0 flat, +1 up).
    pub kronos_direction: f32,
    /// Kronos model confidence (0 = uncertain, 1 = certain).
    pub kronos_confidence: f32,
}

impl RawAlgorithmOutputs {
    /// Normalize features to ~[0,1] or ~[-1,1] for the composite scorer.
    ///
    /// Price-scale features (kalman, swt) are divided by `last_close` so the
    /// dot product is not dominated by absolute price level. Raw fields are
    /// preserved for execution signals (trailing stop, BOCPD exit, VMD trigger).
    ///
    /// NOTE: Both kalman_level and kalman_trend scorer slots intentionally
    /// contain the same `norm_kalman_vel` value. This preserves test baselines.
    pub fn normalize_for_scorer(&self, params: &SignalParams) -> [f32; SCANNER_FEATURE_COUNT] {
        let lc = self.last_close;

        // Kalman trend as fractional velocity (% of price per bar)
        let norm_kalman_vel = if lc.abs() > 1e-10 {
            (self.kalman_trend / lc).clamp(-params.kalman_vel_clamp, params.kalman_vel_clamp)
        } else {
            0.0
        };

        // SWT deviation from close as fraction of price
        let norm_swt = if lc.abs() > 1e-10 {
            ((self.swt_denoised - lc) / lc).clamp(-params.swt_norm_clamp, params.swt_norm_clamp)
                * params.swt_norm_scale
        } else {
            0.0
        };

        // VMD STA/LTA capped to [0, 1]
        let norm_vmd = (self.vmd_sta_lta / params.vmd_norm_divisor).clamp(0.0, 1.0);

        // HMM state: normalize dynamically from hmm_n_states
        let hmm_divisor = (params.hmm_n_states as f32 - 1.0).max(1.0);
        let norm_hmm = self.hmm_state / hmm_divisor;

        [
            self.perm_entropy,
            self.bocpd_prob,
            norm_kalman_vel, // kalman_level slot: fractional velocity (intentional duplicate)
            norm_kalman_vel, // kalman_trend slot: fractional velocity (intentional duplicate)
            norm_vmd,
            self.knn_anomaly,
            self.scatter_class,
            self.vpin,
            self.mp_novelty,
            self.template_corr,
            norm_swt,
            self.hurst,
            norm_hmm,
        ]
    }

    /// Backward-compatible alias for `normalize_for_scorer()`.
    pub fn to_scorer_array(&self, params: &SignalParams) -> [f32; SCANNER_FEATURE_COUNT] {
        self.normalize_for_scorer(params)
    }
}

/// Backward-compatible alias for `RawAlgorithmOutputs`.
pub type ScannerOutput = RawAlgorithmOutputs;

// ---------------------------------------------------------------------------
// InvestigationResult (previously in layer2/mod.rs)
// ---------------------------------------------------------------------------

/// Aggregated output of all investigation algorithms.
///
/// Produced only for candidates that passed the scanner threshold.
/// These values feed into the LLM agent's decision (ENTER / SKIP / WATCHLIST)
/// and the execution layer's position sizing.
#[derive(Clone, Debug)]
pub struct InvestigationResult {
    /// Conformal prediction interval lower offset.
    pub conformal_lo: f32,
    /// Conformal prediction interval upper offset.
    pub conformal_hi: f32,
    /// Renyi transfer entropy at alpha=2 (standard regime coupling).
    pub renyi_te_standard: f32,
    /// Renyi transfer entropy at alpha=5 (tail-regime coupling).
    pub renyi_te_tail: f32,
    /// Kronos model predicted direction (-1 down, 0 flat, +1 up).
    pub kronos_direction: f32,
    /// Kronos model confidence (0 = uncertain, 1 = certain).
    pub kronos_confidence: f32,
}

impl Default for InvestigationResult {
    fn default() -> Self {
        Self {
            conformal_lo: f32::NEG_INFINITY,
            conformal_hi: f32::INFINITY,
            renyi_te_standard: 0.0,
            renyi_te_tail: 0.0,
            kronos_direction: 0.0,
            kronos_confidence: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// SignalOutput
// ---------------------------------------------------------------------------

/// Complete output of one pass through the signal pipeline.
#[derive(Clone, Debug, Default)]
pub struct SignalOutput {
    /// Raw algorithm outputs (scanner features).
    pub scanner: RawAlgorithmOutputs,

    /// Composite scorer output (dot product of features and weights, through
    /// sigmoid). Range approximately [0, 1].
    pub score: f32,

    /// Whether the score exceeds `params.candidate_threshold`.
    pub is_candidate: bool,

    /// Conformal prediction lower bound (present only for candidates).
    pub conformal_lo: Option<f32>,

    /// Conformal prediction upper bound (present only for candidates).
    pub conformal_hi: Option<f32>,

    /// Kalman trailing stop level (ATR-scaled distance below price).
    pub kalman_stop: f32,

    /// Whether BOCPD probability exceeds the exit threshold, signaling
    /// regime death.
    pub bocpd_exit: bool,

    /// Whether VMD STA/LTA crossed the entry threshold, confirming
    /// breakout energy release.
    pub vmd_entry_trigger: bool,
}

// ---------------------------------------------------------------------------
// Characterization (previously layer0::characterize)
// ---------------------------------------------------------------------------

/// Recompute characterization if enough bars have elapsed since the
/// previous computation.
///
/// Returns a new `CharacterizationState`. If the recompute interval has not
/// elapsed, returns a clone of `prev` unchanged.
pub fn characterize(
    close: &[f32],
    returns: &[f32],
    all_returns: Option<(&[f32], usize, usize)>,
    bar_idx: usize,
    params: &SignalParams,
    prev: &CharacterizationState,
) -> CharacterizationState {
    // Not time yet -- return previous state.
    if bar_idx.saturating_sub(prev.last_updated) < params.l0_recompute_interval {
        return prev.clone();
    }

    // --- Hurst exponent ---
    let h = if close.len() >= params.hurst_window {
        hurst::compute_hurst(close, params.hurst_window)
    } else {
        prev.hurst
    };

    // --- HMM regime state ---
    let hmm_model = hmm::HmmModel::from_params(params);
    let hmm_s = hmm::compute_hmm_state(returns, &hmm_model);

    // --- RMT correlation filtering (multi-asset) ---
    let rmt_result = all_returns.map(|(r, nt, nb)| rmt::compute_rmt_filtered(r, nt, nb));

    CharacterizationState {
        hurst: h,
        hmm_state: hmm_s,
        cleaned_corr: rmt_result.as_ref().and_then(|r| {
            if r.filtered_corr.is_empty() {
                None
            } else {
                Some(r.filtered_corr.clone())
            }
        }),
        te_scores: None, // TE computed separately per-pair on demand
        last_updated: bar_idx,
    }
}

// ---------------------------------------------------------------------------
// Scanner (previously layer1::run_scanner)
// ---------------------------------------------------------------------------

/// Run all scanner algorithms for one bar on one ticker.
///
/// Calls each of the 10 sub-algorithms (+ 2 pass-throughs from
/// characterization) and collects the 13 features into `RawAlgorithmOutputs`.
/// Stateful algorithms (Kalman, BOCPD) are updated in-place via the arena.
fn run_scanner(
    close_window: &[f32],
    volume_window: &[f32],
    arena: &mut ThreadArena,
    params: &SignalParams,
    hurst_val: f32,
    hmm_state_val: u8,
) -> RawAlgorithmOutputs {
    let w = close_window.len();
    if w < 2 {
        return RawAlgorithmOutputs::default();
    }
    let last_close = close_window[w - 1];

    // 1.1 Permutation entropy
    let pe = perm_entropy::compute_perm_entropy(close_window, params.pe_order, params.pe_delay);

    // 1.2 BOCPD (stateful -- fed the latest close-to-close return)
    let ret = if w >= 2 {
        close_window[w - 1] - close_window[w - 2]
    } else {
        0.0
    };
    let bocpd_prob = arena.bocpd_state.update(ret, params.bocpd_lambda);

    // 1.3 Kalman filter (stateful -- [level, trend])
    let (k_level, k_trend) = kalman::kalman_update(
        &mut arena.kalman_x,
        &mut arena.kalman_p,
        last_close,
        params.kalman_q,
        params.kalman_r,
    );

    // 1.4 VMD STA/LTA
    let vmd_val = vmd::compute_vmd_sta_lta(
        close_window,
        params.vmd_k_modes,
        params.vmd_alpha,
        params.vmd_sta_window,
        params.vmd_lta_window,
        params.vmd_n_iter,
        params.vmd_tau,
        &mut arena.fft_scratch,
    );

    // 1.5 KNN anomaly (needs historical feature matrix -- neutral for now)
    let knn = 0.0_f32;

    // 1.6 Scattering (needs trained classifier weights -- neutral for now)
    let scatter = 0.5_f32;

    // 1.7 VPIN
    let vpin_score = if !volume_window.is_empty() && volume_window.len() == w {
        vpin::compute_vpin(close_window, volume_window, params.vpin_n_buckets)
    } else {
        0.0
    };

    // 1.8 STOMP matrix profile novelty
    let m = params.stomp_m.min(w);
    let mp_dist = if w > m && m >= 2 {
        stomp::compute_stomp(
            close_window,
            &close_window[w - m..],
            m,
            &mut arena.fft_scratch,
        )
    } else {
        f32::MAX
    };
    // Invert distance to novelty score in [0, 1].
    let mp_novelty = if (0.0..f32::MAX).contains(&mp_dist) {
        1.0 / (1.0 + mp_dist)
    } else {
        0.0
    };

    // 1.9 Template matching (needs template bank -- neutral for now)
    let template = 0.0_f32;

    // 1.10 SWT denoise
    let swt_val = swt::compute_swt_denoise(close_window, params.swt_level, &mut arena.fft_scratch);

    // Store raw values -- normalization happens in normalize_for_scorer().
    RawAlgorithmOutputs {
        perm_entropy: pe,
        bocpd_prob,
        kalman_level: k_level,
        kalman_trend: k_trend,
        vmd_sta_lta: vmd_val,
        knn_anomaly: knn,
        scatter_class: scatter,
        vpin: vpin_score,
        mp_novelty,
        template_corr: template,
        swt_denoised: swt_val,
        hurst: hurst_val,
        hmm_state: hmm_state_val as f32,
        last_close,
        // Investigation fields default to neutral
        conformal_lo: f32::NEG_INFINITY,
        conformal_hi: f32::INFINITY,
        renyi_te_standard: 0.0,
        renyi_te_tail: 0.0,
        kronos_direction: 0.0,
        kronos_confidence: 0.0,
    }
}

// ---------------------------------------------------------------------------
// Investigation (previously layer2::investigate)
// ---------------------------------------------------------------------------

/// Run investigation algorithms on a candidate.
///
/// Only called when scanner score > threshold. Computes:
/// - Conformal prediction interval from the calibrator's historical errors
/// - Renyi TE at alpha=2 and alpha=5 (if cross-asset returns available)
/// - Kronos forecast (stub: neutral until pyo3 bridge is wired)
pub fn investigate(
    close: &[f32],
    source_returns: Option<&[f32]>,
    _scanner: &RawAlgorithmOutputs,
    params: &SignalParams,
    calibrator: &conformal::ConformalCalibrator,
) -> InvestigationResult {
    let (lo, hi) = calibrator.predict_interval(params.conformal_coverage);

    let (rte_std, rte_tail) = if let Some(src) = source_returns {
        let std = renyi::compute_renyi_te(src, close, 2.0, params.knn_k, 1);
        let tail = renyi::compute_renyi_te(src, close, 5.0, params.knn_k, 1);
        (std, tail)
    } else {
        (0.0, 0.0)
    };

    let (dir, _mag, conf) = kronos::kronos_forecast(close);

    InvestigationResult {
        conformal_lo: lo,
        conformal_hi: hi,
        renyi_te_standard: rte_std,
        renyi_te_tail: rte_tail,
        kronos_direction: dir,
        kronos_confidence: conf,
    }
}

// ---------------------------------------------------------------------------
// Pipeline entry point
// ---------------------------------------------------------------------------

/// Run the full signal pipeline for one bar of one ticker.
///
/// Orchestrates: scanner -> composite scorer -> conditional investigation
/// -> execution signals. Characterization is handled externally by the
/// strategy (recomputed every `l0_recompute_interval` bars and passed in
/// via `l0_state`).
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

    // Scanner -- 13 features
    let scanner = run_scanner(
        close_window,
        vol_window,
        arena,
        params,
        l0_state.hurst,
        l0_state.hmm_state,
    );

    // Composite score (normalized features for the dot-product scorer)
    let features = scanner.normalize_for_scorer(params);
    let score = scorer::compute_score(&features, &arena.scorer_weights, arena.scorer_bias);
    let is_candidate = score > params.candidate_threshold;

    // Investigation (conditional -- only for candidates).
    // For MVP, conformal calibrator requires historical residuals that we
    // don't accumulate yet, so intervals are left as None.
    let (conformal_lo, conformal_hi) = (None, None);

    // Execution signals -- reuses scanner state
    let atr = simple_atr(close_window, params.atr_period);
    let conformal_width = match (conformal_lo, conformal_hi) {
        (Some(lo), Some(hi)) => Some(hi - lo),
        _ => None,
    };
    let exec = execution_signals::execution_signals(
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

// ---------------------------------------------------------------------------
// Pattern pipeline entry point
// ---------------------------------------------------------------------------

/// Run fuzzy pattern scoring across four timeframes for one bar of one ticker.
///
/// Returns a `PatternScores` array `[daily, 1h, 30m, 5m]` where each value
/// is a bull flag confidence in [0, 1], or NaN if there is insufficient data.
///
/// Each slice is the full OHLCV column up to and including the current bar.
/// Pass empty slices for timeframes that are unavailable.
pub fn run_pattern_pipeline(
    // Daily OHLCV (ending at current daily bar)
    daily_close: &[f32],
    daily_high: &[f32],
    daily_low: &[f32],
    daily_volume: &[f32],
    daily_atr: f32,
    // 1h OHLCV
    h1_close: &[f32],
    h1_high: &[f32],
    h1_low: &[f32],
    h1_volume: &[f32],
    h1_atr: f32,
    // 30m OHLCV
    m30_close: &[f32],
    m30_high: &[f32],
    m30_low: &[f32],
    m30_volume: &[f32],
    m30_atr: f32,
    // 5m OHLCV
    m5_close: &[f32],
    m5_high: &[f32],
    m5_low: &[f32],
    m5_volume: &[f32],
    m5_atr: f32,
    params: &PatternParams,
) -> PatternScores {
    let score_tf = |c: &[f32], h: &[f32], l: &[f32], v: &[f32], atr: f32| -> f32 {
        if c.is_empty() || h.is_empty() || l.is_empty() || v.is_empty() {
            return f32::NAN;
        }
        engine_patterns::bull_flag_confidence(c, h, l, v, atr, params)
    };

    [
        score_tf(daily_close, daily_high, daily_low, daily_volume, daily_atr),
        score_tf(h1_close, h1_high, h1_low, h1_volume, h1_atr),
        score_tf(m30_close, m30_high, m30_low, m30_volume, m30_atr),
        score_tf(m5_close, m5_high, m5_low, m5_volume, m5_atr),
    ]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_neutral() {
        let state = CharacterizationState::default();
        assert_eq!(state.hurst, 0.5);
        assert_eq!(state.hmm_state, 0);
        assert!(state.cleaned_corr.is_none());
        assert!(state.te_scores.is_none());
        assert_eq!(state.last_updated, 0);
    }

    #[test]
    fn characterize_skips_when_interval_not_elapsed() {
        let prev = CharacterizationState {
            hurst: 0.6,
            hmm_state: 2,
            cleaned_corr: None,
            te_scores: None,
            last_updated: 50,
        };

        let close = vec![100.0; 600];
        let returns = vec![0.001; 600];
        let params = SignalParams::default();

        let result = characterize(&close, &returns, None, 80, &params, &prev);
        // Only 30 bars elapsed (80 - 50 < 100), so should return prev.
        assert_eq!(result.hurst, 0.6);
        assert_eq!(result.hmm_state, 2);
        assert_eq!(result.last_updated, 50);
    }

    #[test]
    fn characterize_recomputes_when_interval_elapsed() {
        let prev = CharacterizationState::default();
        let close: Vec<f32> = (0..600).map(|i| 100.0 + i as f32 * 0.05).collect();
        let returns: Vec<f32> = (0..600).map(|_| 0.001).collect();
        let params = SignalParams::default();

        let result = characterize(&close, &returns, None, 200, &params, &prev);
        assert_eq!(result.last_updated, 200);
        // Hurst should be recomputed (trending data).
        assert!(result.hurst > 0.0);
    }
}
