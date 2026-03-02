//! Composite scorer -- dot-product + sigmoid over the scanner feature vector.
//!
//! The weights and bias are learned offline (logistic regression on labelled
//! pre-breakout / non-breakout windows) and supplied via `SignalParams`.

use crate::signals::feature::SCANNER_FEATURE_COUNT;

/// Compute composite score: sigmoid(dot(features, weights) + bias).
///
/// Output is in (0, 1).  Values above `SignalParams::candidate_threshold`
/// cause the LLM agent to be woken for investigation (Layer 2).
pub fn compute_score(
    features: &[f32; SCANNER_FEATURE_COUNT],
    weights: &[f32; SCANNER_FEATURE_COUNT],
    bias: f32,
) -> f32 {
    let dot: f32 = features.iter().zip(weights.iter()).map(|(f, w)| f * w).sum();
    sigmoid(dot + bias)
}

/// Standard logistic sigmoid, clamped for numerical safety.
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x.clamp(-88.0, 88.0)).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_bias_uniform_weights() {
        let features = [0.5_f32; SCANNER_FEATURE_COUNT];
        let weights = [1.0 / SCANNER_FEATURE_COUNT as f32; SCANNER_FEATURE_COUNT];
        let score = compute_score(&features, &weights, 0.0);
        // dot = 0.5, sigmoid(0.5) ~ 0.622
        assert!((score - 0.622).abs() < 0.01);
    }

    #[test]
    fn extreme_negative_saturates() {
        let features = [-10.0_f32; SCANNER_FEATURE_COUNT];
        let weights = [1.0_f32; SCANNER_FEATURE_COUNT];
        let score = compute_score(&features, &weights, 0.0);
        assert!(score < 0.001);
    }
}
