//! Retrospective breakout detector: "did a significant move already happen?"
//!
//! Unlike `bull_flag` (which scores setup quality before a breakout), this
//! module scores whether a breakout has already occurred in the last N bars.
//! Use it to evaluate trade performance or label training data.
//!
//! # Three components
//!
//! 1. **Return magnitude**: `(close[-1] - close[-N]) / close[-N]` through sigmoid.
//! 2. **Volume surge**: `mean_vol_recent / mean_vol_prior` through sigmoid.
//! 3. **Range expansion**: `mean_ATR_recent / mean_ATR_prior` through sigmoid.
//!    (ATR approximated from high-low range when true ATR isn't available.)
//!
//! All backward-looking, all causal. Output: confidence in [0.0, 1.0].

use engine_types::pattern::BreakoutDetectParams;

use crate::membership::fast_sigmoid;

/// Score whether a breakout occurred in the last `lookback` bars.
///
/// Returns 0.0 for insufficient data, otherwise a fuzzy confidence in [0, 1].
///
/// All slices must end at the current bar (inclusive). They don't need to be
/// the same length — only the most recent `lookback + prior_window` bars
/// are used from each.
pub fn breakout_detected(
    close: &[f32],
    high: &[f32],
    low: &[f32],
    volume: &[f32],
    params: &BreakoutDetectParams,
) -> f32 {
    let n = close.len();
    let lookback = params.detect_lookback;
    let prior = params.detect_prior_window;
    let total_needed = lookback + prior;

    if n < total_needed + 1 || high.len() < total_needed + 1 || low.len() < total_needed + 1 {
        return 0.0;
    }

    let recent_start = n - lookback;
    let prior_start = n - total_needed;
    let prior_end = recent_start; // exclusive

    // -- Component 1: Return magnitude --------------------------------------
    let close_now = close[n - 1];
    let close_before = close[recent_start];

    let score_return = if close_before > 0.0 && !close_before.is_nan() && !close_now.is_nan() {
        let ret = (close_now - close_before) / close_before;
        fast_sigmoid(ret, params.detect_return_mid, params.detect_return_k)
    } else {
        0.0
    };

    // -- Component 2: Volume surge ------------------------------------------
    let score_vol = {
        let recent_vol = mean_valid(&volume[recent_start..n]);
        let prior_vol = mean_valid(&volume[prior_start..prior_end]);
        if prior_vol > 0.0 {
            let ratio = recent_vol / prior_vol;
            fast_sigmoid(ratio, params.detect_vol_surge_mid, params.detect_vol_surge_k)
        } else {
            0.5 // neutral
        }
    };

    // -- Component 3: Range expansion (H-L proxy for ATR) -------------------
    let score_range = {
        let recent_range = mean_range(&high[recent_start..n], &low[recent_start..n]);
        let prior_range = mean_range(&high[prior_start..prior_end], &low[prior_start..prior_end]);
        if prior_range > 0.0 {
            let ratio = recent_range / prior_range;
            fast_sigmoid(ratio, params.detect_range_mid, params.detect_range_k)
        } else {
            0.5
        }
    };

    // -- Weighted mean ------------------------------------------------------
    let [w0, w1, w2] = params.detect_weights;
    let total_w = w0 + w1 + w2;
    if total_w <= 0.0 {
        return 0.0;
    }
    (w0 * score_return + w1 * score_vol + w2 * score_range) / total_w
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Mean of non-NaN, positive values in a slice. Returns 0.0 if none valid.
fn mean_valid(v: &[f32]) -> f32 {
    let mut sum = 0.0_f32;
    let mut count = 0usize;
    for &x in v {
        if !x.is_nan() && x > 0.0 {
            sum += x;
            count += 1;
        }
    }
    if count > 0 { sum / count as f32 } else { 0.0 }
}

/// Mean high-low range over a window (ATR proxy).
fn mean_range(high: &[f32], low: &[f32]) -> f32 {
    let mut sum = 0.0_f32;
    let mut count = 0usize;
    for (&h, &l) in high.iter().zip(low.iter()) {
        if !h.is_nan() && !l.is_nan() {
            sum += h - l;
            count += 1;
        }
    }
    if count > 0 { sum / count as f32 } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_params() -> BreakoutDetectParams {
        BreakoutDetectParams::default()
    }

    /// Clear breakout: price jumps 15% over 10 bars with high volume.
    #[test]
    fn strong_breakout_scores_high() {
        let n = 40;
        let mut close = vec![100.0_f32; n];
        let mut high = vec![101.0_f32; n];
        let mut low = vec![99.0_f32; n];
        let mut volume = vec![500.0_f32; n];

        // Last 10 bars: price ramps from 100 to 115, volume doubles, range expands
        for i in 0..10 {
            let idx = n - 10 + i;
            close[idx] = 100.0 + i as f32 * 1.5;
            high[idx] = close[idx] + 2.0;
            low[idx] = close[idx] - 1.0;
            volume[idx] = 1200.0;
        }

        let params = default_params();
        let score = breakout_detected(&close, &high, &low, &volume, &params);
        assert!(score > 0.6, "expected >0.6, got {score}");
    }

    /// Flat price action: should score low.
    #[test]
    fn flat_market_scores_low() {
        let n = 40;
        let close = vec![100.0_f32; n];
        let high = vec![100.5_f32; n];
        let low = vec![99.5_f32; n];
        let volume = vec![500.0_f32; n];

        let params = default_params();
        let score = breakout_detected(&close, &high, &low, &volume, &params);
        assert!(score < 0.5, "expected <0.5 for flat market, got {score}");
    }

    /// Insufficient data returns 0.
    #[test]
    fn short_data_returns_zero() {
        let close = vec![100.0_f32; 5];
        let high = vec![101.0_f32; 5];
        let low = vec![99.0_f32; 5];
        let volume = vec![500.0_f32; 5];

        let params = default_params();
        let score = breakout_detected(&close, &high, &low, &volume, &params);
        assert_eq!(score, 0.0);
    }
}
