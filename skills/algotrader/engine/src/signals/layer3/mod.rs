//! Layer 3 -- Execution signals.
//!
//! Active only while a position is open.  Reuses Layer 1 state (Kalman
//! filter, BOCPD run lengths, VMD decomposition) to produce adaptive stop
//! levels, regime-death exit triggers, precise entry timing, and
//! confidence-scaled position sizing.
//!
//! No new heavy computation here -- everything is derived from values
//! already computed by the scanner and investigation layers.

use super::params::SignalParams;

/// Execution-layer signals computed from Layer 1 state.
///
/// These reuse Kalman, BOCPD, and VMD state -- no new computation needed.
#[derive(Clone, Debug)]
pub struct ExecutionSignals {
    /// Adaptive trailing stop: kalman_level - mult * atr.
    /// NaN if inputs are unavailable.
    pub kalman_stop: f32,
    /// True if BOCPD detects regime death (P(cp) > threshold).
    pub bocpd_exit: bool,
    /// True if VMD STA/LTA crosses entry threshold (breakout confirmed).
    pub vmd_entry_trigger: bool,
    /// Position size scaling from conformal interval width.
    /// 1.0 = normal, >1 = high confidence, <1 = low confidence.
    pub conformal_size_scale: f32,
}

impl Default for ExecutionSignals {
    fn default() -> Self {
        Self {
            kalman_stop: f32::NAN,
            bocpd_exit: false,
            vmd_entry_trigger: false,
            conformal_size_scale: 1.0,
        }
    }
}

/// Compute execution signals from Layer 1 scanner output.
///
/// # Signals
///
/// - **kalman_stop** (3.1): adaptive trailing stop that widens in strong
///   uptrends and tightens when trend weakens.  Computed as
///   `kalman_level - atr_mult * trend_factor * atr`.
///
/// - **bocpd_exit** (3.2): regime death detector.  True when P(changepoint)
///   exceeds the exit threshold, meaning the regime that produced the entry
///   has ended.
///
/// - **vmd_entry_trigger** (3.4): breakout confirmation.  True when VMD
///   STA/LTA exceeds the entry threshold, signaling volatility expansion
///   on the clean swing mode.
///
/// - **conformal_size_scale** (3.3): position size modifier from conformal
///   prediction interval width.  Narrow interval = high confidence = scale
///   up (capped at 3x).  Wide interval = low confidence = scale down
///   (floored at 0.1x).
pub fn execution_signals(
    kalman_level: f32,
    kalman_trend: f32,
    bocpd_prob: f32,
    vmd_sta_lta: f32,
    atr: f32,
    conformal_width: Option<f32>,
    params: &SignalParams,
) -> ExecutionSignals {
    // 3.1 Kalman trailing stop -- adaptive based on trend strength.
    //
    // Strong uptrend (positive velocity) -> wider stop to let the trade run.
    // Weakening/negative trend -> tighter stop to protect gains.
    let trend_factor = if kalman_trend > 0.0 {
        // Scale up to 2x for strong trends (capped).
        1.0 + (kalman_trend.abs() * 100.0).min(1.0)
    } else {
        // Scale down to 0.5x for weakening/reversing trends.
        (1.0 - (kalman_trend.abs() * 100.0).min(0.5)).max(0.5)
    };

    let kalman_stop = if atr.is_nan() || kalman_level.is_nan() {
        f32::NAN
    } else {
        kalman_level - params.kalman_stop_atr_mult * trend_factor * atr
    };

    // 3.2 BOCPD exit -- regime death detection.
    // If the changepoint probability exceeds the threshold while in a
    // position, the regime that justified the entry has ended.
    let bocpd_exit = bocpd_prob > params.bocpd_exit_threshold;

    // 3.4 VMD entry trigger -- volatility expansion on the swing mode.
    // The agent decided ENTER based on coiling (high compression), but we
    // wait for STA/LTA to cross the threshold for precise entry timing.
    let vmd_entry_trigger = vmd_sta_lta > params.vmd_entry_threshold;

    // 3.3 Conformal position sizer -- scale by inverse interval width.
    // Narrow interval = model is confident = allow larger position.
    // Wide interval = uncertain = reduce position size.
    let conformal_size_scale = match conformal_width {
        Some(w) if w > 0.0 && w.is_finite() => (1.0 / w).clamp(0.1, 3.0),
        _ => 1.0,
    };

    ExecutionSignals {
        kalman_stop,
        bocpd_exit,
        vmd_entry_trigger,
        conformal_size_scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals::params::SignalParams;

    #[test]
    fn default_has_nan_stop() {
        let es = ExecutionSignals::default();
        assert!(es.kalman_stop.is_nan());
        assert!(!es.bocpd_exit);
        assert!(!es.vmd_entry_trigger);
        assert_eq!(es.conformal_size_scale, 1.0);
    }

    #[test]
    fn kalman_stop_basic() {
        let params = SignalParams::default();
        let es = execution_signals(100.0, 0.005, 0.1, 1.0, 2.0, None, &params);
        // trend_factor = 1.0 + min(0.5, 1.0) = 1.5
        // stop = 100.0 - 2.0 * 1.5 * 2.0 = 100 - 6 = 94
        assert!((es.kalman_stop - 94.0).abs() < 0.01, "got {}", es.kalman_stop);
    }

    #[test]
    fn kalman_stop_negative_trend_tighter() {
        let params = SignalParams::default();
        let es = execution_signals(100.0, -0.005, 0.1, 1.0, 2.0, None, &params);
        // trend_factor = max(1.0 - min(0.5, 0.5), 0.5) = 0.5
        // stop = 100.0 - 2.0 * 0.5 * 2.0 = 100 - 2 = 98
        assert!((es.kalman_stop - 98.0).abs() < 0.01, "got {}", es.kalman_stop);
    }

    #[test]
    fn kalman_stop_nan_inputs() {
        let params = SignalParams::default();
        let es = execution_signals(f32::NAN, 0.0, 0.0, 0.0, 1.0, None, &params);
        assert!(es.kalman_stop.is_nan());

        let es2 = execution_signals(100.0, 0.0, 0.0, 0.0, f32::NAN, None, &params);
        assert!(es2.kalman_stop.is_nan());
    }

    #[test]
    fn bocpd_exit_threshold() {
        let params = SignalParams::default(); // threshold = 0.7
        assert!(!execution_signals(100.0, 0.0, 0.5, 0.0, 1.0, None, &params).bocpd_exit);
        assert!(!execution_signals(100.0, 0.0, 0.7, 0.0, 1.0, None, &params).bocpd_exit);
        assert!(execution_signals(100.0, 0.0, 0.71, 0.0, 1.0, None, &params).bocpd_exit);
    }

    #[test]
    fn vmd_entry_trigger() {
        let params = SignalParams::default(); // threshold = 2.0
        assert!(!execution_signals(100.0, 0.0, 0.0, 1.9, 1.0, None, &params).vmd_entry_trigger);
        assert!(!execution_signals(100.0, 0.0, 0.0, 2.0, 1.0, None, &params).vmd_entry_trigger);
        assert!(execution_signals(100.0, 0.0, 0.0, 2.1, 1.0, None, &params).vmd_entry_trigger);
    }

    #[test]
    fn conformal_size_scale_narrow() {
        let params = SignalParams::default();
        // Narrow interval (width=0.5) -> scale = 2.0
        let es = execution_signals(100.0, 0.0, 0.0, 0.0, 1.0, Some(0.5), &params);
        assert!((es.conformal_size_scale - 2.0).abs() < 0.01);
    }

    #[test]
    fn conformal_size_scale_wide() {
        let params = SignalParams::default();
        // Wide interval (width=100.0) -> scale = 0.01, clamped to 0.1
        let es = execution_signals(100.0, 0.0, 0.0, 0.0, 1.0, Some(100.0), &params);
        assert!((es.conformal_size_scale - 0.1).abs() < 0.01);
    }

    #[test]
    fn conformal_size_scale_none() {
        let params = SignalParams::default();
        let es = execution_signals(100.0, 0.0, 0.0, 0.0, 1.0, None, &params);
        assert_eq!(es.conformal_size_scale, 1.0);
    }

    #[test]
    fn conformal_size_scale_zero_width() {
        let params = SignalParams::default();
        let es = execution_signals(100.0, 0.0, 0.0, 0.0, 1.0, Some(0.0), &params);
        assert_eq!(es.conformal_size_scale, 1.0);
    }

    #[test]
    fn conformal_size_capped_at_three() {
        let params = SignalParams::default();
        // Very narrow interval (width=0.1) -> 1/0.1 = 10, capped to 3.0
        let es = execution_signals(100.0, 0.0, 0.0, 0.0, 1.0, Some(0.1), &params);
        assert!((es.conformal_size_scale - 3.0).abs() < 0.01);
    }
}
