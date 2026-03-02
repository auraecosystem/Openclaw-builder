//! 2.1 Conformal Prediction -- distribution-free uncertainty quantification.
//!
//! Maintains a rolling buffer of nonconformity scores (|predicted - actual|)
//! and computes prediction intervals at a given coverage level.  The interval
//! width drives position sizing: narrow = confident = larger size.

/// Conformal prediction calibration state.
///
/// Accumulates nonconformity scores from historical predictions and uses
/// their empirical quantile to produce prediction intervals with
/// guaranteed finite-sample coverage.
#[derive(Clone, Debug)]
pub struct ConformalCalibrator {
    /// Absolute nonconformity scores from historical predictions.
    scores: Vec<f32>,
    /// Maximum number of scores to retain (rolling window).
    max_size: usize,
}

impl ConformalCalibrator {
    /// Create a new calibrator with the given rolling window size.
    pub fn new(max_size: usize) -> Self {
        Self {
            scores: Vec::with_capacity(max_size.min(256)),
            max_size: max_size.max(1),
        }
    }

    /// Add a calibration observation.
    ///
    /// `residual` is the raw prediction error (predicted - actual).
    /// The absolute value is stored as the nonconformity score.
    pub fn add_score(&mut self, residual: f32) {
        if residual.is_nan() {
            return;
        }
        if self.scores.len() >= self.max_size {
            // Drop the oldest score to keep a rolling window.
            self.scores.remove(0);
        }
        self.scores.push(residual.abs());
    }

    /// Compute a symmetric prediction interval at the given coverage level.
    ///
    /// Returns `(lower_offset, upper_offset)` to be applied as
    /// `prediction - offset` and `prediction + offset`.
    ///
    /// Coverage should be in (0, 1) -- e.g. 0.90 for 90% coverage.
    /// If fewer than 10 calibration scores are available, returns a wide
    /// default interval to reflect high uncertainty.
    pub fn predict_interval(&self, coverage: f32) -> (f32, f32) {
        let n = self.scores.len();

        // Not enough data -- return a conservatively wide interval.
        if n < 10 {
            return (f32::NEG_INFINITY, f32::INFINITY);
        }

        let alpha = 1.0 - coverage.clamp(0.01, 0.99);

        // Quantile index: ceil((n+1) * (1-alpha)) clamped to valid range.
        let q_idx = (((n + 1) as f32) * (1.0 - alpha)).ceil() as usize;
        let q_idx = q_idx.min(n).saturating_sub(1);

        // Partial sort to find the quantile without full sort.
        let mut buf = self.scores.clone();
        buf.select_nth_unstable_by(q_idx, |a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let offset = buf[q_idx];

        (-offset, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_sample_returns_wide_interval() {
        let cal = ConformalCalibrator::new(200);
        let (lo, hi) = cal.predict_interval(0.90);
        assert!(lo == f32::NEG_INFINITY);
        assert!(hi == f32::INFINITY);
    }

    #[test]
    fn basic_coverage() {
        let mut cal = ConformalCalibrator::new(200);
        for i in 1..=100 {
            cal.add_score(i as f32 * 0.1);
        }
        let (lo, hi) = cal.predict_interval(0.90);
        assert!(lo < 0.0);
        assert!(hi > 0.0);
        assert!(hi > -lo - 0.001); // symmetric
    }

    #[test]
    fn nan_scores_ignored() {
        let mut cal = ConformalCalibrator::new(200);
        cal.add_score(f32::NAN);
        assert_eq!(cal.scores.len(), 0);
    }

    #[test]
    fn rolling_window_caps_size() {
        let mut cal = ConformalCalibrator::new(20);
        for i in 0..50 {
            cal.add_score(i as f32);
        }
        assert_eq!(cal.scores.len(), 20);
    }
}
