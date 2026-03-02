//! Unit tests for Layer 1 scanner algorithms.
//!
//! Covers ScannerOutput, Permutation Entropy, Kalman Filter, Scorer, BOCPD,
//! and VPIN.  Each test validates the documented contract without relying on
//! internal state.

use algotrader_engine::signals::feature::SCANNER_FEATURE_COUNT;
use algotrader_engine::signals::layer1::ScannerOutput;
use algotrader_engine::signals::layer1::perm_entropy;
use algotrader_engine::signals::layer1::kalman;
use algotrader_engine::signals::layer1::scorer;
use algotrader_engine::signals::layer1::bocpd::BocpdState;
use algotrader_engine::signals::layer1::vpin;

// ---------------------------------------------------------------------------
// ScannerOutput
// ---------------------------------------------------------------------------

#[test]
fn scorer_array_length() {
    let s = ScannerOutput::default();
    let arr = s.to_scorer_array();
    assert_eq!(arr.len(), SCANNER_FEATURE_COUNT);
    assert_eq!(arr.len(), 13);
}

#[test]
fn scorer_array_default_is_zeros() {
    let s = ScannerOutput::default();
    let arr = s.to_scorer_array();
    for (i, val) in arr.iter().enumerate() {
        assert!(
            (val - 0.0).abs() < 1e-6,
            "scorer default arr[{i}] should be 0, got {val}",
        );
    }
}

#[test]
fn scorer_array_normalizes_price_scale_features() {
    // Raw kalman_trend = 50, last_close = 50000 → normalized = 0.001
    // Raw swt_denoised = 50050, last_close = 50000 → (50/50000)*10 = 0.01
    // Raw vmd_sta_lta = 3.0 → 3.0/5.0 = 0.6
    // Raw hmm_state = 2.0 → 2.0/2.0 = 1.0
    let s = ScannerOutput {
        perm_entropy: 0.5,
        bocpd_prob: 0.3,
        kalman_level: 50000.0,
        kalman_trend: 50.0,
        vmd_sta_lta: 3.0,
        knn_anomaly: 0.0,
        scatter_class: 0.5,
        vpin: 0.4,
        mp_novelty: 0.2,
        template_corr: 0.0,
        swt_denoised: 50050.0,
        hurst: 0.6,
        hmm_state: 2.0,
        last_close: 50000.0,
    };
    let arr = s.to_scorer_array();

    // All values should be in a reasonable normalized range
    for (i, &val) in arr.iter().enumerate() {
        assert!(
            val.abs() <= 1.1,
            "scorer arr[{i}] should be normalized, got {val}",
        );
    }

    // Specific checks
    assert!((arr[0] - 0.5).abs() < 1e-6, "perm_entropy unchanged: {}", arr[0]);
    assert!((arr[1] - 0.3).abs() < 1e-6, "bocpd_prob unchanged: {}", arr[1]);
    // kalman slots: 50.0/50000.0 = 0.001
    assert!(arr[2].abs() < 0.01, "kalman normalized: {}", arr[2]);
    assert!(arr[3].abs() < 0.01, "kalman normalized: {}", arr[3]);
    // vmd: 3.0/5.0 = 0.6
    assert!((arr[4] - 0.6).abs() < 1e-6, "vmd normalized: {}", arr[4]);
    // hmm: 2.0/2.0 = 1.0
    assert!((arr[12] - 1.0).abs() < 1e-6, "hmm normalized: {}", arr[12]);
}

// ---------------------------------------------------------------------------
// Permutation Entropy (1.1)
// ---------------------------------------------------------------------------

#[test]
fn pe_ordered_sequence_low_entropy() {
    // Perfectly ascending sequence has only one ordinal pattern: PE near 0.
    let ordered: Vec<f32> = (0..50).map(|i| i as f32).collect();
    let pe = perm_entropy::compute_perm_entropy(&ordered, 5, 1);
    assert!(pe < 0.1, "ordered sequence should have low PE, got {pe}");
}

#[test]
fn pe_descending_sequence_low_entropy() {
    // Perfectly descending also has a single dominant pattern.
    let descending: Vec<f32> = (0..50).rev().map(|i| i as f32).collect();
    let pe = perm_entropy::compute_perm_entropy(&descending, 5, 1);
    assert!(pe < 0.1, "descending sequence should have low PE, got {pe}");
}

#[test]
fn pe_short_input_returns_max() {
    // Fewer elements than one embedding vector: max entropy (1.0).
    let short = vec![1.0, 2.0];
    let pe = perm_entropy::compute_perm_entropy(&short, 5, 1);
    assert!((pe - 1.0).abs() < 0.01, "short input should return ~1.0, got {pe}");
}

#[test]
fn pe_order_two_minimum() {
    // Order 2 with sufficient data should compute.
    let data: Vec<f32> = (0..30).map(|i| i as f32).collect();
    let pe = perm_entropy::compute_perm_entropy(&data, 2, 1);
    assert!((0.0..=1.0).contains(&pe), "PE should be in [0,1], got {pe}");
}

#[test]
fn pe_order_one_returns_max() {
    // Order < 2 is degenerate.
    let data: Vec<f32> = (0..30).map(|i| i as f32).collect();
    let pe = perm_entropy::compute_perm_entropy(&data, 1, 1);
    assert!((pe - 1.0).abs() < 0.01, "order=1 should return max, got {pe}");
}

#[test]
fn pe_output_in_unit_interval() {
    // Arbitrary signal: output must be clamped to [0, 1].
    let data: Vec<f32> = (0..100)
        .map(|i| 50.0 + (i as f32 * 0.7).sin() * 10.0)
        .collect();
    let pe = perm_entropy::compute_perm_entropy(&data, 5, 1);
    assert!((0.0..=1.0).contains(&pe), "PE should be in [0,1], got {pe}");
}

#[test]
fn pe_delay_two_still_works() {
    // With delay=2, embedding is [x[0], x[2], x[4], ...].
    let data: Vec<f32> = (0..60).map(|i| i as f32).collect();
    let pe = perm_entropy::compute_perm_entropy(&data, 5, 2);
    assert!(pe < 0.1, "ascending with delay=2 should have low PE, got {pe}");
}

// ---------------------------------------------------------------------------
// Kalman Filter (1.3)
// ---------------------------------------------------------------------------

#[test]
fn kalman_tracks_constant_level() {
    let mut x = [100.0, 0.0];
    let mut p = [1.0, 0.0, 0.0, 1.0];
    for _ in 0..50 {
        kalman::kalman_update(&mut x, &mut p, 100.0, 0.001, 1.0);
    }
    assert!(
        (x[0] - 100.0).abs() < 1.0,
        "kalman should track constant, got {}",
        x[0],
    );
    assert!(
        x[1].abs() < 0.1,
        "trend should be near 0 for constant, got {}",
        x[1],
    );
}

#[test]
fn kalman_tracks_uptrend() {
    let mut x = [100.0, 0.0];
    let mut p = [1.0, 0.0, 0.0, 1.0];
    for i in 0..100 {
        kalman::kalman_update(&mut x, &mut p, 100.0 + i as f32, 0.001, 1.0);
    }
    assert!(x[1] > 0.0, "trend should be positive for uptrend, got {}", x[1]);
}

#[test]
fn kalman_tracks_downtrend() {
    let mut x = [200.0, 0.0];
    let mut p = [1.0, 0.0, 0.0, 1.0];
    for i in 0..100 {
        kalman::kalman_update(&mut x, &mut p, 200.0 - i as f32, 0.001, 1.0);
    }
    assert!(x[1] < 0.0, "trend should be negative for downtrend, got {}", x[1]);
}

#[test]
fn kalman_returns_level_and_trend() {
    let mut x = [50.0, 0.0];
    let mut p = [1.0, 0.0, 0.0, 1.0];
    let (level, trend) = kalman::kalman_update(&mut x, &mut p, 55.0, 0.001, 1.0);
    assert!(level > 50.0, "level should move toward observation, got {level}");
    assert!(trend.is_finite(), "trend should be finite, got {trend}");
    // State vector should match return values.
    assert!((x[0] - level).abs() < 1e-6);
    assert!((x[1] - trend).abs() < 1e-6);
}

#[test]
fn kalman_high_r_slow_tracking() {
    // High measurement noise R means slower convergence.
    let mut x_fast = [0.0_f32; 2];
    let mut p_fast = [1.0, 0.0, 0.0, 1.0];
    let mut x_slow = [0.0_f32; 2];
    let mut p_slow = [1.0, 0.0, 0.0, 1.0];

    for _ in 0..10 {
        kalman::kalman_update(&mut x_fast, &mut p_fast, 100.0, 0.001, 1.0);
        kalman::kalman_update(&mut x_slow, &mut p_slow, 100.0, 0.001, 100.0);
    }
    // Fast (low R) should be closer to 100 than slow (high R).
    assert!(
        (x_fast[0] - 100.0).abs() < (x_slow[0] - 100.0).abs(),
        "low R should converge faster: fast={}, slow={}",
        x_fast[0],
        x_slow[0],
    );
}

// ---------------------------------------------------------------------------
// Scorer (1.x)
// ---------------------------------------------------------------------------

#[test]
fn scorer_all_zeros_returns_sigmoid_of_bias_zero() {
    let features = [0.0f32; SCANNER_FEATURE_COUNT];
    let weights = [1.0f32; SCANNER_FEATURE_COUNT];
    let score = scorer::compute_score(&features, &weights, 0.0);
    assert!(
        (score - 0.5).abs() < 1e-6,
        "sigmoid(0) should be 0.5, got {score}",
    );
}

#[test]
fn scorer_high_features_high_score() {
    let features = [1.0f32; SCANNER_FEATURE_COUNT];
    let weights = [1.0f32; SCANNER_FEATURE_COUNT];
    let score = scorer::compute_score(&features, &weights, 0.0);
    assert!(
        score > 0.99,
        "sigmoid(13) should be near 1.0, got {score}",
    );
}

#[test]
fn scorer_negative_features_low_score() {
    let features = [-10.0f32; SCANNER_FEATURE_COUNT];
    let weights = [1.0f32; SCANNER_FEATURE_COUNT];
    let score = scorer::compute_score(&features, &weights, 0.0);
    assert!(score < 0.001, "sigmoid(-130) should be near 0, got {score}");
}

#[test]
fn scorer_positive_bias_shifts_score_up() {
    let features = [0.0f32; SCANNER_FEATURE_COUNT];
    let weights = [1.0f32; SCANNER_FEATURE_COUNT];
    let score_no_bias = scorer::compute_score(&features, &weights, 0.0);
    let score_with_bias = scorer::compute_score(&features, &weights, 5.0);
    assert!(
        score_with_bias > score_no_bias,
        "positive bias should increase score: no_bias={score_no_bias}, with_bias={score_with_bias}",
    );
}

#[test]
fn scorer_output_in_unit_interval() {
    // Extreme inputs should still produce output in (0, 1).
    let features = [88.0f32; SCANNER_FEATURE_COUNT];
    let weights = [1.0f32; SCANNER_FEATURE_COUNT];
    let score = scorer::compute_score(&features, &weights, 0.0);
    assert!(score > 0.0 && score < 1.0 + 1e-6, "score should be in (0,1), got {score}");
}

// ---------------------------------------------------------------------------
// BOCPD (1.2)
// ---------------------------------------------------------------------------

#[test]
fn bocpd_first_update_returns_zero() {
    // The first observation initialises state; no changepoint is possible.
    let mut state = BocpdState::new(200);
    let cp = state.update(0.0, 100.0);
    assert!(
        (cp - 0.0).abs() < 1e-6,
        "first update should return 0.0, got {cp}",
    );
}

#[test]
fn bocpd_detects_mean_shift() {
    // 50 bars near 0, then 50 bars near 5 -- a clear regime shift.
    // Use a shorter lambda (20) for higher hazard rate so the detector
    // responds within the window.
    let mut state = BocpdState::new(200);
    let mut max_cp = 0.0_f32;
    for i in 0..100 {
        let x = if i < 50 {
            0.1 * (i % 5) as f32
        } else {
            5.0 + 0.1 * (i % 5) as f32
        };
        let cp = state.update(x, 20.0);
        if (50..=60).contains(&i) {
            max_cp = max_cp.max(cp);
        }
    }
    assert!(max_cp > 0.001, "BOCPD should detect mean shift, max P(cp)={max_cp}");
}

#[test]
fn bocpd_stable_regime_low_changepoint() {
    // A long stable regime should have low changepoint probability.
    let mut state = BocpdState::new(200);
    let mut max_cp = 0.0_f32;
    for i in 0..200 {
        let cp = state.update(0.1 * (i % 3) as f32, 100.0);
        if i > 50 {
            max_cp = max_cp.max(cp);
        }
    }
    // After stabilising, P(cp) should be low.
    assert!(max_cp < 0.5, "stable regime should have low P(cp), got {max_cp}");
}

#[test]
fn bocpd_nan_input_returns_zero() {
    let mut state = BocpdState::new(200);
    state.update(0.0, 100.0); // init
    let cp = state.update(f32::NAN, 100.0);
    assert!((cp - 0.0).abs() < 1e-6, "NaN input should return 0.0, got {cp}");
}

#[test]
fn bocpd_reset_clears_state() {
    let mut state = BocpdState::new(200);
    for _ in 0..50 {
        state.update(1.0, 100.0);
    }
    state.reset();
    // After reset, first update should behave like initial.
    let cp = state.update(0.0, 100.0);
    assert!((cp - 0.0).abs() < 1e-6, "after reset, first update should return 0, got {cp}");
}

#[test]
fn bocpd_output_clamped() {
    // Changepoint probability should always be in [0, 1].
    let mut state = BocpdState::new(200);
    for _ in 0..100 {
        state.update(0.0, 100.0);
    }
    for _ in 0..50 {
        let cp = state.update(10.0, 100.0);
        assert!(
            (0.0..=1.0).contains(&cp),
            "P(cp) should be in [0,1], got {cp}",
        );
    }
}

// ---------------------------------------------------------------------------
// VPIN (1.7)
// ---------------------------------------------------------------------------

#[test]
fn vpin_all_buys_max_imbalance() {
    // Strictly increasing close: all bars classified as buys.
    let close: Vec<f32> = (0..20).map(|i| 100.0 + i as f32).collect();
    let volume = vec![100.0; 20];
    let v = vpin::compute_vpin(&close, &volume, 5);
    assert!((v - 1.0).abs() < 0.05, "all-buy should give VPIN near 1.0, got {v}");
}

#[test]
fn vpin_alternating_low_imbalance() {
    // Alternating up/down should produce low VPIN.
    let mut close = vec![100.0_f32; 20];
    for (i, c) in close.iter_mut().enumerate().take(20).skip(1) {
        *c = if i % 2 == 0 { 101.0 } else { 99.0 };
    }
    let volume = vec![100.0; 20];
    let v = vpin::compute_vpin(&close, &volume, 5);
    assert!(v < 0.5, "alternating should give low VPIN, got {v}");
}

#[test]
fn vpin_too_short_returns_zero() {
    let v = vpin::compute_vpin(&[100.0], &[100.0], 5);
    assert!((v - 0.0).abs() < 1e-6, "single bar should return 0, got {v}");
}

#[test]
fn vpin_empty_returns_zero() {
    let v = vpin::compute_vpin(&[], &[], 5);
    assert!((v - 0.0).abs() < 1e-6, "empty should return 0, got {v}");
}

#[test]
fn vpin_mismatched_lengths_returns_zero() {
    let v = vpin::compute_vpin(&[100.0, 101.0], &[100.0], 5);
    assert!((v - 0.0).abs() < 1e-6, "mismatched lengths should return 0, got {v}");
}

#[test]
fn vpin_zero_volume_returns_zero() {
    let close = vec![100.0, 101.0, 102.0];
    let volume = vec![0.0; 3];
    let v = vpin::compute_vpin(&close, &volume, 5);
    assert!((v - 0.0).abs() < 1e-6, "zero volume should return 0, got {v}");
}

#[test]
fn vpin_zero_buckets_returns_zero() {
    let close = vec![100.0, 101.0, 102.0];
    let volume = vec![100.0; 3];
    let v = vpin::compute_vpin(&close, &volume, 0);
    assert!((v - 0.0).abs() < 1e-6, "zero buckets should return 0, got {v}");
}

#[test]
fn vpin_output_in_unit_interval() {
    // Arbitrary data: output must be in [0, 1].
    let close: Vec<f32> = (0..50)
        .map(|i| 100.0 + (i as f32 * 0.3).sin() * 5.0)
        .collect();
    let volume = vec![500.0; 50];
    let v = vpin::compute_vpin(&close, &volume, 10);
    assert!((0.0..=1.0).contains(&v), "VPIN should be in [0,1], got {v}");
}
