//! Causal bull flag fuzzy confidence scorer.
//!
//! # Algorithm (no lookahead)
//!
//! 1. Scan backward `flag_lookback` bars from current bar for the highest high
//!    → this is the pole peak.
//! 2. Find the lowest close before the peak (within lookback) → pole start.
//! 3. Compute pole return: `(peak - trough) / trough`.
//! 4. Compute flag region: bars from peak to current bar (up to flag_max_bars).
//! 5. Score 4 components via membership functions.
//! 6. Return weighted mean.
//!
//! # Difference from Python `detect_swing_points`
//!
//! The Python version uses future bars to confirm swing points. This version
//! uses only a rolling max/min in a fixed lookback window, making it safe for
//! real-time and backtest use without lookahead bias.

use engine_types::PatternParams;

use crate::membership::{cauchy_membership, fast_sigmoid, trapezoidal};

/// Compute causal bull flag confidence for the current bar.
///
/// Returns a value in [0.0, 1.0], or 0.0 if there is insufficient data
/// for meaningful scoring.
///
/// # Arguments
///
/// - `close`: full close price column ending at current bar
/// - `high`: full high price column (same length as close)
/// - `low`: full low price column (same length as close)
/// - `volume`: full volume column (same length as close)
/// - `atr`: average true range at current bar (used for slope normalization)
/// - `params`: fuzzy pattern parameters
pub fn bull_flag_confidence(
    close: &[f32],
    high: &[f32],
    low: &[f32],
    volume: &[f32],
    atr: f32,
    params: &PatternParams,
) -> f32 {
    let n = close.len();
    if n < params.flag_pole_min_bars + 2 {
        return 0.0;
    }

    // -- Step 1: Find pole peak (highest high in lookback window) -----------

    let lookback_start = n.saturating_sub(params.flag_lookback);
    let search_end = n - 1; // exclude current bar from pole peak search

    let pole_peak_idx = (lookback_start..search_end)
        .filter(|&i| !high[i].is_nan())
        .max_by(|&a, &b| high[a].partial_cmp(&high[b]).unwrap_or(std::cmp::Ordering::Equal));

    let pole_peak_idx = match pole_peak_idx {
        Some(i) => i,
        None => return 0.0,
    };
    let pole_peak = high[pole_peak_idx];

    // -- Step 2: Find pole trough (lowest close before peak) ---------------

    let pole_start_idx = (lookback_start..pole_peak_idx)
        .filter(|&i| !close[i].is_nan())
        .min_by(|&a, &b| close[a].partial_cmp(&close[b]).unwrap_or(std::cmp::Ordering::Equal));

    let pole_start_idx = match pole_start_idx {
        Some(i) => i,
        None => return 0.0,
    };
    let pole_trough = close[pole_start_idx];

    // Pole must have at least `flag_pole_min_bars` duration
    if pole_peak_idx.saturating_sub(pole_start_idx) < params.flag_pole_min_bars {
        return 0.0;
    }

    if pole_trough <= 0.0 || pole_trough.is_nan() || pole_peak.is_nan() {
        return 0.0;
    }

    // -- Step 3: Pole return ------------------------------------------------

    let pole_return = (pole_peak - pole_trough) / pole_trough;

    // -- Step 4: Flag region (pole peak to current bar) --------------------

    let flag_start = pole_peak_idx + 1;
    let flag_end = n; // exclusive, current bar is included
    let flag_len = flag_end.saturating_sub(flag_start);

    if flag_len == 0 || flag_len > params.flag_max_bars {
        return 0.0;
    }

    let flag_close = &close[flag_start..flag_end];
    let flag_high = &high[flag_start..flag_end];
    let flag_low = &low[flag_start..flag_end];
    let pole_volume = &volume[pole_start_idx..flag_start];
    let flag_volume = &volume[flag_start..flag_end];

    // -- Component 1: Pole strength -----------------------------------------
    let score_pole = fast_sigmoid(pole_return, params.flag_pole_mid, params.flag_pole_k);

    // -- Component 2: Flag slope (linear regression slope / ATR) -----------
    let score_slope = if flag_len >= 2 && !atr.is_nan() && atr > 0.0 {
        let slope_per_bar = linear_regression_slope(flag_close);
        let norm_slope = slope_per_bar / atr; // slope in ATR units per bar
        cauchy_membership(norm_slope, params.flag_slope_center, params.flag_slope_sigma)
    } else {
        0.5 // neutral if insufficient data
    };

    // -- Component 3: Tightness (retracement fraction of pole height) ------
    let pole_height = pole_peak - pole_trough;
    let score_retrace = if pole_height > 0.0 {
        let flag_high_val = flag_high
            .iter()
            .copied()
            .filter(|v| !v.is_nan())
            .fold(f32::NEG_INFINITY, f32::max);
        let flag_low_val = flag_low
            .iter()
            .copied()
            .filter(|v| !v.is_nan())
            .fold(f32::INFINITY, f32::min);
        let retrace_range = if flag_high_val.is_finite() && flag_low_val.is_finite() {
            flag_high_val - flag_low_val
        } else {
            pole_height * 0.5 // fallback: treat as 50% retrace
        };
        let retrace_frac = retrace_range / pole_height;
        trapezoidal(retrace_frac, params.flag_retrace_lo, params.flag_retrace_hi)
    } else {
        0.0
    };

    // -- Component 4: Volume profile (flag vol should be < pole vol) -------
    let score_vol = {
        let pole_avg_vol = mean_volume(pole_volume);
        let flag_avg_vol = mean_volume(flag_volume);
        if pole_avg_vol > 0.0 {
            let ratio = flag_avg_vol / pole_avg_vol;
            // Negative k: lower ratio → higher sigmoid output
            fast_sigmoid(ratio, params.flag_vol_mid, params.flag_vol_k)
        } else {
            0.5 // neutral
        }
    };

    // -- Weighted mean ------------------------------------------------------
    let [w0, w1, w2, w3] = params.flag_weights;
    let total_w = w0 + w1 + w2 + w3;
    if total_w <= 0.0 {
        return 0.0;
    }
    (w0 * score_pole + w1 * score_slope + w2 * score_retrace + w3 * score_vol) / total_w
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Ordinary least squares slope of a slice (bars are equally spaced, x=0..n).
///
/// Returns 0.0 for slices shorter than 2 elements.
fn linear_regression_slope(y: &[f32]) -> f32 {
    let n = y.len();
    if n < 2 {
        return 0.0;
    }
    let nf = n as f32;
    // x_mean = (n-1)/2, y_mean computed below
    let x_mean = (nf - 1.0) * 0.5;
    let mut y_sum = 0.0_f32;
    let mut valid = 0usize;
    for &v in y {
        if !v.is_nan() {
            y_sum += v;
            valid += 1;
        }
    }
    if valid < 2 {
        return 0.0;
    }
    let y_mean = y_sum / valid as f32;
    let mut num = 0.0_f32;
    let mut den = 0.0_f32;
    for (i, &yi) in y.iter().enumerate() {
        if yi.is_nan() {
            continue;
        }
        let xi = i as f32;
        num += (xi - x_mean) * (yi - y_mean);
        den += (xi - x_mean) * (xi - x_mean);
    }
    if den.abs() < 1e-12 {
        0.0
    } else {
        num / den
    }
}

/// Mean of a volume slice, ignoring NaN and zero values.
fn mean_volume(v: &[f32]) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_params() -> PatternParams {
        PatternParams::default()
    }

    /// Synthetic perfect bull flag: strong pole, tight horizontal consolidation,
    /// declining volume. Should score > 0.6.
    #[test]
    fn perfect_bull_flag_scores_high() {
        let mut close = Vec::new();
        let mut high = Vec::new();
        let mut low = Vec::new();
        let mut volume = Vec::new();

        // 20-bar pole: strong uptrend
        for i in 0..20usize {
            let p = 100.0 + i as f32 * 1.5;
            close.push(p);
            high.push(p + 0.5);
            low.push(p - 0.3);
            volume.push(1000.0 + i as f32 * 20.0);
        }
        // 10-bar flag: slight downward drift, tight range, low volume
        for i in 0..10usize {
            let p = 129.0 - i as f32 * 0.2;
            close.push(p);
            high.push(p + 0.3);
            low.push(p - 0.2);
            volume.push(300.0);
        }

        let params = make_params();
        let atr = 1.0;
        let confidence = bull_flag_confidence(&close, &high, &low, &volume, atr, &params);
        assert!(confidence > 0.5, "expected >0.5, got {confidence}");
    }

    /// Random noise should score low (< 0.5 on average).
    #[test]
    fn noise_scores_low() {
        // Flat + noisy, no real pole
        let n = 50;
        let close: Vec<f32> = (0..n).map(|i| 100.0 + ((i * 7) % 5) as f32 * 0.1 - 0.2).collect();
        let high: Vec<f32> = close.iter().map(|&c| c + 0.1).collect();
        let low: Vec<f32> = close.iter().map(|&c| c - 0.1).collect();
        let volume: Vec<f32> = vec![500.0; n];

        let params = make_params();
        let confidence = bull_flag_confidence(&close, &high, &low, &volume, 0.2, &params);
        // Noise should not trigger a confident bull flag
        assert!(confidence < 0.8, "expected <0.8, got {confidence}");
    }

    /// Too short a series returns 0.0 gracefully.
    #[test]
    fn insufficient_data_returns_zero() {
        let close = vec![100.0_f32, 101.0, 102.0];
        let high = vec![100.5_f32, 101.5, 102.5];
        let low = vec![99.5_f32, 100.5, 101.5];
        let vol = vec![500.0_f32, 500.0, 500.0];
        let params = make_params();
        let confidence = bull_flag_confidence(&close, &high, &low, &vol, 0.5, &params);
        assert_eq!(confidence, 0.0);
    }
}
