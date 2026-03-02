//! Layer 2 -- Agent investigation.
//!
//! Called only when the composite scanner score exceeds the candidate
//! threshold.  Algorithms here (conformal prediction, Renyi transfer
//! entropy, foundation model forecast) are more expensive and provide
//! the context packet for the LLM agent's go/no-go decision.

pub mod conformal;
pub mod kronos;
pub mod renyi;

use super::layer1::ScannerOutput;
use super::params::SignalParams;

/// Aggregated output of all Layer 2 investigation algorithms.
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

/// Run investigation layer on a candidate.
///
/// Only called when scanner score > threshold.  Computes:
/// - Conformal prediction interval from the calibrator's historical errors
/// - Renyi TE at alpha=2 and alpha=5 (if cross-asset returns available)
/// - Kronos forecast (stub: neutral until pyo3 bridge is wired)
pub fn investigate(
    close: &[f32],
    source_returns: Option<&[f32]>,
    _scanner: &ScannerOutput,
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
