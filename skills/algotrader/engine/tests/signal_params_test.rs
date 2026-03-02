//! Unit tests for `algotrader_engine::signals::params::SignalParams`.
//!
//! Validates default construction, serde round-tripping, and deserialization
//! from partial JSON (all fields should get their serde defaults).

use algotrader_engine::signals::params::SignalParams;
use algotrader_engine::signals::feature::SCANNER_FEATURE_COUNT;

// ---------------------------------------------------------------------------
// Default construction
// ---------------------------------------------------------------------------

#[test]
fn default_params_are_valid() {
    let p = SignalParams::default();
    assert!(p.window_len > 0, "window_len should be positive, got {}", p.window_len);
    assert!(
        p.candidate_threshold > 0.0 && p.candidate_threshold < 1.0,
        "candidate_threshold should be in (0,1), got {}",
        p.candidate_threshold,
    );
    assert!(p.pe_order >= 3, "pe_order should be >= 3, got {}", p.pe_order);
    assert!(p.bocpd_lambda > 0.0, "bocpd_lambda should be positive, got {}", p.bocpd_lambda);
    assert!(p.kalman_q > 0.0, "kalman_q should be positive, got {}", p.kalman_q);
    assert!(p.kalman_r > 0.0, "kalman_r should be positive, got {}", p.kalman_r);
    assert_eq!(
        p.scorer_weights.len(),
        SCANNER_FEATURE_COUNT,
        "scorer_weights should have {SCANNER_FEATURE_COUNT} elements",
    );
}

#[test]
fn default_window_len_is_200() {
    let p = SignalParams::default();
    assert_eq!(p.window_len, 200);
}

#[test]
fn default_candidate_threshold_is_half() {
    let p = SignalParams::default();
    assert!((p.candidate_threshold - 0.5).abs() < 1e-6);
}

#[test]
fn default_scorer_weights_are_uniform() {
    let p = SignalParams::default();
    let expected = 1.0 / SCANNER_FEATURE_COUNT as f32;
    for (i, &w) in p.scorer_weights.iter().enumerate() {
        assert!(
            (w - expected).abs() < 1e-6,
            "scorer_weights[{i}] = {w}, expected {expected}",
        );
    }
}

#[test]
fn default_scorer_bias_is_zero() {
    let p = SignalParams::default();
    assert!((p.scorer_bias - 0.0).abs() < 1e-6);
}

#[test]
fn default_l0_recompute_interval() {
    let p = SignalParams::default();
    assert_eq!(p.l0_recompute_interval, 100);
}

#[test]
fn default_execution_thresholds() {
    let p = SignalParams::default();
    assert!((p.kalman_stop_atr_mult - 2.0).abs() < 1e-6);
    assert!((p.bocpd_exit_threshold - 0.7).abs() < 1e-6);
    assert!((p.vmd_entry_threshold - 2.0).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// Serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn params_serde_roundtrip() {
    let p = SignalParams::default();
    let json = serde_json::to_string(&p).unwrap();
    let p2: SignalParams = serde_json::from_str(&json).unwrap();
    assert_eq!(p.window_len, p2.window_len);
    assert!((p.candidate_threshold - p2.candidate_threshold).abs() < 1e-6);
    assert_eq!(p.pe_order, p2.pe_order);
    assert!((p.bocpd_lambda - p2.bocpd_lambda).abs() < 1e-6);
    assert!((p.kalman_q - p2.kalman_q).abs() < 1e-6);
    assert!((p.kalman_r - p2.kalman_r).abs() < 1e-6);
    assert!((p.scorer_bias - p2.scorer_bias).abs() < 1e-6);
    for (i, (&a, &b)) in p.scorer_weights.iter().zip(p2.scorer_weights.iter()).enumerate() {
        assert!((a - b).abs() < 1e-6, "weight[{i}] mismatch: {a} vs {b}");
    }
}

#[test]
fn params_serde_roundtrip_preserves_all_fields() {
    let p = SignalParams::default();
    let json = serde_json::to_string_pretty(&p).unwrap();
    let p2: SignalParams = serde_json::from_str(&json).unwrap();
    assert_eq!(p.vmd_k_modes, p2.vmd_k_modes);
    assert!((p.vmd_alpha - p2.vmd_alpha).abs() < 1e-6);
    assert_eq!(p.knn_k, p2.knn_k);
    assert_eq!(p.scatter_j, p2.scatter_j);
    assert_eq!(p.stomp_m, p2.stomp_m);
    assert_eq!(p.template_len, p2.template_len);
    assert_eq!(p.swt_level, p2.swt_level);
    assert_eq!(p.hurst_window, p2.hurst_window);
    assert_eq!(p.hmm_n_states, p2.hmm_n_states);
    assert_eq!(p.rmt_window, p2.rmt_window);
    assert!((p.conformal_coverage - p2.conformal_coverage).abs() < 1e-6);
    assert_eq!(p.conformal_window, p2.conformal_window);
    assert_eq!(p.vpin_n_buckets, p2.vpin_n_buckets);
}

// ---------------------------------------------------------------------------
// Deserialization from partial / empty JSON
// ---------------------------------------------------------------------------

#[test]
fn params_from_empty_json() {
    // All defaults should apply when deserializing an empty object.
    let p: SignalParams = serde_json::from_str("{}").unwrap();
    assert_eq!(p.window_len, 200);
    assert!((p.candidate_threshold - 0.5).abs() < 1e-6);
    assert_eq!(p.pe_order, 5);
    assert!((p.bocpd_lambda - 100.0).abs() < 1e-6);
}

#[test]
fn params_from_partial_json_uses_defaults_for_missing() {
    let json = r#"{"window_len": 300, "pe_order": 4}"#;
    let p: SignalParams = serde_json::from_str(json).unwrap();
    // Overridden fields:
    assert_eq!(p.window_len, 300);
    assert_eq!(p.pe_order, 4);
    // Default fields:
    assert!((p.candidate_threshold - 0.5).abs() < 1e-6);
    assert!((p.bocpd_lambda - 100.0).abs() < 1e-6);
    assert!((p.kalman_q - 0.001).abs() < 1e-6);
}
