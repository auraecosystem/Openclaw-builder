//! Pattern composite scorer: weighted sum of PatternScores across timeframes.

/// Compute the composite pattern score from per-timeframe confidence values.
///
/// `scores`: slice of `[f32; N]` pattern confidence values
/// `weights`: per-feature weights (same length as scores)
/// `bias`: additive bias applied before sigmoid
///
/// Returns a value in [0, 1] via a fast sigmoid approximation.
pub fn compute_pattern_score(scores: &[f32], weights: &[f32], bias: f32) -> f32 {
    debug_assert_eq!(scores.len(), weights.len());
    let mut sum = bias;
    for (&s, &w) in scores.iter().zip(weights.iter()) {
        if !s.is_nan() {
            sum += w * s;
        }
    }
    // Fast sigmoid: 0.5 + 0.5 * z / (1 + |z|)
    0.5 + 0.5 * sum / (1.0 + sum.abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_bias_equal_weights_neutral() {
        // Zero scores + zero bias → weighted sum = 0 → fast_sigmoid(0) = 0.5 exactly.
        // (Scores of 0.5 produce sum=0.5 → output=0.667; use score=0 for true neutrality.)
        let scores = [0.0_f32, 0.0, 0.0, 0.0];
        let weights = [0.25_f32; 4];
        let s = compute_pattern_score(&scores, &weights, 0.0);
        assert!((s - 0.5).abs() < 1e-6, "expected 0.5, got {s}");
    }

    #[test]
    fn high_scores_produce_high_output() {
        let scores = [0.9_f32; 4];
        let weights = [0.25_f32; 4];
        let s = compute_pattern_score(&scores, &weights, 0.0);
        assert!(s > 0.5, "expected >0.5, got {s}");
    }
}
