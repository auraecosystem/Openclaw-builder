//! Unit tests for Layer 3 execution signals.
//!
//! Tests the `execution_signals()` function which derives adaptive stops,
//! regime-death exits, breakout triggers, and confidence-based sizing from
//! Layer 1 scanner state.

use algotrader_engine::signals::layer3;
use algotrader_engine::signals::layer3::ExecutionSignals;
use algotrader_engine::signals::params::SignalParams;

// ---------------------------------------------------------------------------
// ExecutionSignals default
// ---------------------------------------------------------------------------

#[test]
fn execution_signals_default_has_nan_stop() {
    let es = ExecutionSignals::default();
    assert!(es.kalman_stop.is_nan(), "default kalman_stop should be NaN");
}

#[test]
fn execution_signals_default_no_exit() {
    let es = ExecutionSignals::default();
    assert!(!es.bocpd_exit, "default bocpd_exit should be false");
}

#[test]
fn execution_signals_default_no_trigger() {
    let es = ExecutionSignals::default();
    assert!(!es.vmd_entry_trigger, "default vmd_entry_trigger should be false");
}

#[test]
fn execution_signals_default_scale_one() {
    let es = ExecutionSignals::default();
    assert!(
        (es.conformal_size_scale - 1.0).abs() < 1e-6,
        "default conformal_size_scale should be 1.0, got {}",
        es.conformal_size_scale,
    );
}

// ---------------------------------------------------------------------------
// BOCPD exit threshold
// ---------------------------------------------------------------------------

#[test]
fn execution_bocpd_exit_fires_above_threshold() {
    let params = SignalParams::default(); // bocpd_exit_threshold = 0.7
    let es = layer3::execution_signals(100.0, 0.01, 0.8, 1.5, 2.0, None, &params);
    assert!(es.bocpd_exit, "should exit when bocpd_prob=0.8 > threshold=0.7");
}

#[test]
fn execution_bocpd_no_exit_below_threshold() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(100.0, 0.01, 0.3, 1.5, 2.0, None, &params);
    assert!(!es.bocpd_exit, "should not exit when bocpd_prob=0.3 < threshold=0.7");
}

#[test]
fn execution_bocpd_no_exit_at_threshold() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(100.0, 0.01, 0.7, 1.5, 2.0, None, &params);
    assert!(!es.bocpd_exit, "should not exit when bocpd_prob=0.7 == threshold (strict >)");
}

// ---------------------------------------------------------------------------
// VMD entry trigger
// ---------------------------------------------------------------------------

#[test]
fn execution_vmd_trigger_fires_above_threshold() {
    let params = SignalParams::default(); // vmd_entry_threshold = 2.0
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 3.0, 2.0, None, &params);
    assert!(es.vmd_entry_trigger, "should trigger when sta_lta=3.0 > threshold=2.0");
}

#[test]
fn execution_vmd_trigger_no_fire_below_threshold() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 1.5, 2.0, None, &params);
    assert!(!es.vmd_entry_trigger, "should not trigger when sta_lta=1.5 < threshold=2.0");
}

#[test]
fn execution_vmd_trigger_no_fire_at_threshold() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 2.0, 2.0, None, &params);
    assert!(!es.vmd_entry_trigger, "should not trigger when sta_lta=2.0 == threshold (strict >)");
}

// ---------------------------------------------------------------------------
// Kalman trailing stop
// ---------------------------------------------------------------------------

#[test]
fn execution_kalman_stop_basic() {
    let params = SignalParams::default(); // kalman_stop_atr_mult = 2.0
    let es = layer3::execution_signals(100.0, 0.0, 0.1, 1.0, 5.0, None, &params);
    // trend = 0.0, trend_factor varies by implementation.
    // Stop should be below the kalman level.
    assert!(
        es.kalman_stop < 100.0,
        "kalman stop should be below level, got {}",
        es.kalman_stop,
    );
    assert!(
        es.kalman_stop.is_finite(),
        "kalman stop should be finite, got {}",
        es.kalman_stop,
    );
}

#[test]
fn execution_kalman_stop_nan_atr() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 1.0, f32::NAN, None, &params);
    assert!(es.kalman_stop.is_nan(), "stop should be NaN when ATR is NaN");
}

#[test]
fn execution_kalman_stop_nan_level() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(f32::NAN, 0.01, 0.1, 1.0, 2.0, None, &params);
    assert!(es.kalman_stop.is_nan(), "stop should be NaN when kalman_level is NaN");
}

#[test]
fn execution_kalman_stop_positive_trend_wider() {
    let params = SignalParams::default();
    // Positive trend should produce a wider stop (lower value = further below level).
    let es_pos = layer3::execution_signals(100.0, 0.005, 0.1, 1.0, 2.0, None, &params);
    let es_neg = layer3::execution_signals(100.0, -0.005, 0.1, 1.0, 2.0, None, &params);
    assert!(
        es_pos.kalman_stop < es_neg.kalman_stop,
        "positive trend should give wider (lower) stop: pos={}, neg={}",
        es_pos.kalman_stop,
        es_neg.kalman_stop,
    );
}

// ---------------------------------------------------------------------------
// Conformal position size scale
// ---------------------------------------------------------------------------

#[test]
fn execution_conformal_scale_default_when_none() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 1.0, 2.0, None, &params);
    assert!(
        (es.conformal_size_scale - 1.0).abs() < 1e-6,
        "no conformal width -> scale=1.0, got {}",
        es.conformal_size_scale,
    );
}

#[test]
fn execution_conformal_narrow_scales_up() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 1.0, 2.0, Some(0.5), &params);
    assert!(
        es.conformal_size_scale > 1.0,
        "narrow interval should scale up, got {}",
        es.conformal_size_scale,
    );
}

#[test]
fn execution_conformal_wide_scales_down() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 1.0, 2.0, Some(100.0), &params);
    assert!(
        es.conformal_size_scale < 1.0,
        "wide interval should scale down, got {}",
        es.conformal_size_scale,
    );
}

#[test]
fn execution_conformal_zero_width_defaults() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 1.0, 2.0, Some(0.0), &params);
    assert!(
        (es.conformal_size_scale - 1.0).abs() < 1e-6,
        "zero width should default to 1.0, got {}",
        es.conformal_size_scale,
    );
}

#[test]
fn execution_conformal_nan_width_defaults() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 1.0, 2.0, Some(f32::NAN), &params);
    assert!(
        (es.conformal_size_scale - 1.0).abs() < 1e-6,
        "NaN width should default to 1.0, got {}",
        es.conformal_size_scale,
    );
}

#[test]
fn execution_conformal_scale_capped_high() {
    let params = SignalParams::default();
    // Very narrow interval (width=0.01) -> 1/0.01 = 100, should be capped.
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 1.0, 2.0, Some(0.01), &params);
    assert!(
        es.conformal_size_scale <= 3.0 + 1e-6,
        "conformal scale should be capped at 3.0, got {}",
        es.conformal_size_scale,
    );
}

#[test]
fn execution_conformal_scale_floored() {
    let params = SignalParams::default();
    // Very wide interval -> low scale, should be floored.
    let es = layer3::execution_signals(100.0, 0.01, 0.1, 1.0, 2.0, Some(1000.0), &params);
    assert!(
        es.conformal_size_scale >= 0.1 - 1e-6,
        "conformal scale should be floored at 0.1, got {}",
        es.conformal_size_scale,
    );
}

// ---------------------------------------------------------------------------
// Combined scenario: all signals firing
// ---------------------------------------------------------------------------

#[test]
fn execution_all_signals_fire_simultaneously() {
    let params = SignalParams::default();
    let es = layer3::execution_signals(
        100.0,   // kalman_level
        0.01,    // kalman_trend (positive)
        0.9,     // bocpd_prob (above 0.7)
        3.0,     // vmd_sta_lta (above 2.0)
        2.0,     // atr
        Some(0.5), // conformal_width (narrow)
        &params,
    );
    assert!(es.bocpd_exit, "bocpd should fire");
    assert!(es.vmd_entry_trigger, "vmd should fire");
    assert!(es.kalman_stop.is_finite(), "stop should be finite");
    assert!(es.conformal_size_scale > 1.0, "narrow width should scale up");
}
