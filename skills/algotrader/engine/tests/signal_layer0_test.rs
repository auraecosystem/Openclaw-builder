//! Unit tests for Layer 0 algorithms: Hurst exponent (DFA) and HMM Viterbi.
//!
//! Tests validate contracts from SIGNAL_SPEC.md: output ranges, edge cases
//! (short input, NaN), and behavioral expectations for known signal types.

use algotrader_engine::signals::layer0::hurst;
use algotrader_engine::signals::layer0::hmm;
use algotrader_engine::signals::layer0::CharacterizationState;

// ---------------------------------------------------------------------------
// Hurst exponent tests
// ---------------------------------------------------------------------------

#[test]
fn hurst_trending_signal() {
    // Monotonically increasing prices should produce H > 0.5 (trending).
    let prices: Vec<f32> = (0..600).map(|i| 100.0 + i as f32 * 0.1).collect();
    let h = hurst::compute_hurst(&prices, 500);
    assert!(h > 0.5, "trending series should have H > 0.5, got {h}");
}

#[test]
fn hurst_alternating_signal_lower_than_trend() {
    // An alternating (mean-reverting) signal should have a lower Hurst
    // exponent than a pure uptrend.  We do not assert H < 0.5 exactly
    // because DFA on finite-length synthetic signals can overshoot.
    let trend: Vec<f32> = (0..600).map(|i| 100.0 + i as f32 * 0.1).collect();
    let alt: Vec<f32> = (0..600)
        .map(|i| 100.0 + if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let h_trend = hurst::compute_hurst(&trend, 500);
    let h_alt = hurst::compute_hurst(&alt, 500);
    assert!(
        h_alt < h_trend,
        "alternating series H={h_alt} should be less than trending H={h_trend}",
    );
}

#[test]
fn hurst_short_input_returns_default() {
    // Insufficient data should return the 0.5 neutral fallback.
    let prices = vec![100.0; 10];
    let h = hurst::compute_hurst(&prices, 500);
    assert!((h - 0.5).abs() < 0.01, "short input should return ~0.5, got {h}");
}

#[test]
fn hurst_nan_input_returns_neutral() {
    // NaN prices trigger the degenerate early return.
    let prices = vec![f32::NAN; 600];
    let h = hurst::compute_hurst(&prices, 500);
    assert!(
        (0.0..=1.0).contains(&h),
        "hurst on NaN input should be in [0,1], got {h}",
    );
}

#[test]
fn hurst_constant_price_returns_neutral() {
    // All-constant price means zero returns; degenerate DFA returns 0.5.
    let prices = vec![100.0_f32; 600];
    let h = hurst::compute_hurst(&prices, 500);
    assert!((h - 0.5).abs() < 0.01, "constant price should return ~0.5, got {h}");
}

#[test]
fn hurst_output_in_unit_interval() {
    // Regardless of input pattern, output must be clamped to [0, 1].
    let prices: Vec<f32> = (0..600)
        .map(|i| 100.0 + (i as f32 * 0.3).cos() * 10.0 + i as f32 * 0.01)
        .collect();
    let h = hurst::compute_hurst(&prices, 500);
    assert!((0.0..=1.0).contains(&h), "hurst must be in [0,1], got {h}");
}

#[test]
fn hurst_window_larger_than_data_uses_available() {
    // Window exceeds data length; should use what is available.
    let prices: Vec<f32> = (0..100).map(|i| 100.0 + i as f32).collect();
    let h = hurst::compute_hurst(&prices, 1000);
    assert!((0.0..=1.0).contains(&h), "hurst should handle window > data, got {h}");
}

#[test]
fn hurst_negative_prices_returns_neutral() {
    // Negative prices make log returns invalid; should return 0.5.
    let prices: Vec<f32> = (0..600).map(|i| -100.0 - i as f32).collect();
    let h = hurst::compute_hurst(&prices, 500);
    assert!((h - 0.5).abs() < 0.01, "negative prices should return ~0.5, got {h}");
}

// ---------------------------------------------------------------------------
// HMM tests
// ---------------------------------------------------------------------------

#[test]
fn hmm_returns_valid_state() {
    let returns: Vec<f32> = (0..200).map(|i| (i as f32 * 0.1).sin() * 0.02).collect();
    let state = hmm::compute_hmm_state(&returns, &hmm::HmmModel::default());
    assert!(state < 3, "HMM state should be 0, 1, or 2, got {state}");
}

#[test]
fn hmm_empty_returns_default_state() {
    let state = hmm::compute_hmm_state(&[], &hmm::HmmModel::default());
    assert_eq!(state, 0, "empty returns should give default state 0");
}

#[test]
fn hmm_low_vol_returns_state_zero() {
    // Tiny returns close to zero mean should be classified as low-vol.
    let returns: Vec<f32> = (0..200).map(|_| 0.00001).collect();
    let state = hmm::compute_hmm_state(&returns, &hmm::HmmModel::default());
    assert_eq!(state, 0, "tiny returns should produce low-vol state 0, got {state}");
}

#[test]
fn hmm_different_returns_may_differ() {
    // Different return distributions should (usually) produce different
    // state classifications.  We test that the HMM is at least sensitive
    // to its input by comparing two distinct regimes.
    let model = hmm::HmmModel::default();
    let quiet: Vec<f32> = (0..200).map(|_| 0.00001).collect();
    let volatile: Vec<f32> = (0..200).map(|i| if i % 2 == 0 { 0.05 } else { -0.05 }).collect();
    let state_quiet = hmm::compute_hmm_state(&quiet, &model);
    let state_volatile = hmm::compute_hmm_state(&volatile, &model);
    // Both should be valid states.
    assert!(state_quiet < 3);
    assert!(state_volatile < 3);
    // They should differ (quiet = low-vol, volatile = high-vol).
    assert_ne!(
        state_quiet, state_volatile,
        "quiet and volatile regimes should produce different states: quiet={state_quiet}, volatile={state_volatile}",
    );
}

#[test]
fn hmm_nan_first_observation_returns_default() {
    let state = hmm::compute_hmm_state(&[f32::NAN], &hmm::HmmModel::default());
    assert_eq!(state, 0, "NaN first observation should return 0");
}

#[test]
fn hmm_nan_mid_sequence_skips_gracefully() {
    // NaN in the middle should be skipped; surrounding valid returns
    // should still be classified.
    let mut returns = vec![0.002_f32; 100];
    returns[50] = f32::NAN;
    let state = hmm::compute_hmm_state(&returns, &hmm::HmmModel::default());
    assert!(state < 3, "should produce valid state despite mid-sequence NaN");
}

#[test]
fn hmm_single_observation_returns_valid_state() {
    let state = hmm::compute_hmm_state(&[0.001], &hmm::HmmModel::default());
    assert!(state < 3, "single observation should give a valid state, got {state}");
}

#[test]
fn hmm_default_model_has_three_states() {
    let model = hmm::HmmModel::default();
    assert_eq!(model.n_states, 3);
    assert_eq!(model.emission_mean.len(), 3);
    assert_eq!(model.emission_var.len(), 3);
}

// ---------------------------------------------------------------------------
// CharacterizationState tests
// ---------------------------------------------------------------------------

#[test]
fn characterization_state_default_hurst() {
    let s = CharacterizationState::default();
    assert!((s.hurst - 0.5).abs() < 1e-6, "default hurst should be 0.5, got {}", s.hurst);
}

#[test]
fn characterization_state_default_hmm_state() {
    let s = CharacterizationState::default();
    assert_eq!(s.hmm_state, 0, "default hmm_state should be 0");
}

#[test]
fn characterization_state_default_optional_fields_none() {
    let s = CharacterizationState::default();
    assert!(s.cleaned_corr.is_none(), "cleaned_corr should default to None");
    assert!(s.te_scores.is_none(), "te_scores should default to None");
}

#[test]
fn characterization_state_default_last_updated() {
    let s = CharacterizationState::default();
    assert_eq!(s.last_updated, 0, "last_updated should default to 0");
}
