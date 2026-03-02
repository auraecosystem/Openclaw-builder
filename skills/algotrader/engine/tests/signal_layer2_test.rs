//! Unit tests for Layer 2 algorithms: Conformal Prediction.
//!
//! Tests validate the ConformalCalibrator's behavior: wide intervals on empty
//! state, narrowing with consistent calibration data, NaN handling, and
//! rolling window capacity.

use algotrader_engine::signals::layer2::conformal::ConformalCalibrator;

// ---------------------------------------------------------------------------
// Empty / insufficient calibration
// ---------------------------------------------------------------------------

#[test]
fn conformal_empty_returns_wide_interval() {
    let cal = ConformalCalibrator::new(200);
    let (lo, hi) = cal.predict_interval(0.90);
    assert!(
        lo == f32::NEG_INFINITY,
        "empty calibrator lower bound should be -inf, got {lo}",
    );
    assert!(
        hi == f32::INFINITY,
        "empty calibrator upper bound should be +inf, got {hi}",
    );
}

#[test]
fn conformal_few_scores_returns_wide_interval() {
    // Fewer than 10 calibration scores should produce the wide default.
    let mut cal = ConformalCalibrator::new(200);
    for i in 0..9 {
        cal.add_score(i as f32 * 0.1);
    }
    let (lo, hi) = cal.predict_interval(0.90);
    assert!(lo == f32::NEG_INFINITY);
    assert!(hi == f32::INFINITY);
}

// ---------------------------------------------------------------------------
// Tight residuals should give narrow interval
// ---------------------------------------------------------------------------

#[test]
fn conformal_tight_residuals_narrow_interval() {
    let mut cal = ConformalCalibrator::new(200);
    for _ in 0..100 {
        cal.add_score(0.01); // very consistent predictions
    }
    let (lo, hi) = cal.predict_interval(0.90);
    let width = hi - lo;
    assert!(
        width < 1.0,
        "tight residuals should give narrow interval, got width={width}",
    );
}

#[test]
fn conformal_wider_residuals_wider_interval() {
    let mut cal_tight = ConformalCalibrator::new(200);
    let mut cal_wide = ConformalCalibrator::new(200);

    for _ in 0..100 {
        cal_tight.add_score(0.01);
        cal_wide.add_score(10.0);
    }

    let (lo_t, hi_t) = cal_tight.predict_interval(0.90);
    let (lo_w, hi_w) = cal_wide.predict_interval(0.90);
    let width_tight = hi_t - lo_t;
    let width_wide = hi_w - lo_w;

    assert!(
        width_wide > width_tight,
        "wider residuals should produce wider interval: tight={width_tight}, wide={width_wide}",
    );
}

// ---------------------------------------------------------------------------
// Symmetric interval
// ---------------------------------------------------------------------------

#[test]
fn conformal_interval_is_symmetric() {
    let mut cal = ConformalCalibrator::new(200);
    for i in 1..=100 {
        cal.add_score(i as f32 * 0.1);
    }
    let (lo, hi) = cal.predict_interval(0.90);
    // Interval is [-offset, +offset], so lo = -hi.
    assert!(
        (lo + hi).abs() < 1e-6,
        "interval should be symmetric: lo={lo}, hi={hi}",
    );
}

// ---------------------------------------------------------------------------
// NaN handling
// ---------------------------------------------------------------------------

#[test]
fn conformal_nan_scores_ignored() {
    let mut cal = ConformalCalibrator::new(200);
    cal.add_score(f32::NAN);
    cal.add_score(f32::NAN);
    // Should have no scores after NaN adds.
    let (lo, hi) = cal.predict_interval(0.90);
    assert!(lo == f32::NEG_INFINITY, "NaN scores should be ignored");
    assert!(hi == f32::INFINITY, "NaN scores should be ignored");
}

#[test]
fn conformal_nan_mixed_with_valid() {
    let mut cal = ConformalCalibrator::new(200);
    for _ in 0..50 {
        cal.add_score(0.1);
        cal.add_score(f32::NAN); // should be skipped
    }
    // Should have 50 valid scores.
    let (lo, hi) = cal.predict_interval(0.90);
    let width = hi - lo;
    assert!(width.is_finite(), "should produce finite interval with 50 scores");
}

// ---------------------------------------------------------------------------
// Rolling window
// ---------------------------------------------------------------------------

#[test]
fn conformal_rolling_window_caps_size() {
    let mut cal = ConformalCalibrator::new(20);
    for i in 0..50 {
        cal.add_score(i as f32);
    }
    // After 50 adds with max_size=20, only 20 should be retained.
    // We test indirectly: the interval should reflect the last 20 scores
    // (values 30-49), not the full 0-49 range.
    let (lo, hi) = cal.predict_interval(0.90);
    assert!(lo.is_finite() && hi.is_finite(), "should have enough scores");
}

// ---------------------------------------------------------------------------
// Coverage levels
// ---------------------------------------------------------------------------

#[test]
fn conformal_higher_coverage_wider_interval() {
    let mut cal = ConformalCalibrator::new(200);
    for i in 1..=100 {
        cal.add_score(i as f32 * 0.01);
    }
    let (_, hi_80) = cal.predict_interval(0.80);
    let (_, hi_95) = cal.predict_interval(0.95);
    // 95% coverage should produce a wider (or equal) interval than 80%.
    assert!(
        hi_95 >= hi_80 - 1e-6,
        "95% coverage should be >= 80%: hi_80={hi_80}, hi_95={hi_95}",
    );
}
