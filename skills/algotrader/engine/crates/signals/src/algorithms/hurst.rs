//! 0.1 DFA Hurst Exponent
//!
//! Detrended Fluctuation Analysis on log returns.
//! H > 0.55 = trending, H < 0.45 = mean-reverting, ~0.5 = random walk.

/// Compute rolling Hurst exponent via Detrended Fluctuation Analysis on log
/// returns derived from `close` prices.
///
/// Returns H in [0, 1]. Requires at least `window` elements in `close`.
/// On degenerate input (NaN, insufficient data, numerical failure) returns 0.5
/// (random-walk neutral).
pub fn compute_hurst(close: &[f32], window: usize) -> f32 {
    let w = window.min(close.len());
    if w < 16 {
        return 0.5;
    }

    // --- 1. Log returns from trailing window ---
    let tail = &close[close.len() - w..];
    let n = w - 1; // number of returns
    let mut returns = Vec::with_capacity(n);
    for i in 1..=n {
        let prev = tail[i - 1];
        let cur = tail[i];
        if prev <= 0.0 || cur <= 0.0 || prev.is_nan() || cur.is_nan() {
            return 0.5;
        }
        returns.push((cur / prev).ln());
    }

    // --- 2. Demeaned cumulative sum (profile) ---
    let mean = returns.iter().copied().sum::<f32>() / n as f32;
    if mean.is_nan() {
        return 0.5;
    }
    let mut profile = Vec::with_capacity(n);
    let mut cum = 0.0_f32;
    for &r in &returns {
        cum += r - mean;
        profile.push(cum);
    }

    // --- 3. Box sizes: powers of 2 from 4 up to n/4 ---
    let max_box = n / 4;
    if max_box < 4 {
        return 0.5;
    }

    let mut log_s = Vec::new();
    let mut log_f = Vec::new();

    let mut s = 4_usize;
    while s <= max_box {
        let n_boxes = n / s;
        if n_boxes == 0 {
            s *= 2;
            continue;
        }

        let mut total_rms = 0.0_f64;
        for b in 0..n_boxes {
            let start = b * s;
            // Least-squares line fit within box: y = a + b*x
            // Using x = 0..s-1 for simplicity (Gauss method for evenly-spaced x).
            let rms = box_rms(&profile[start..start + s]);
            total_rms += rms as f64;
        }

        let f_s = (total_rms / n_boxes as f64) as f32;
        if f_s > 0.0 && f_s.is_finite() {
            log_s.push((s as f32).ln());
            log_f.push(f_s.ln());
        }

        s *= 2;
    }

    // --- 4. H = slope of log(F(s)) vs log(s) ---
    if log_s.len() < 2 {
        return 0.5;
    }
    let slope = least_squares_slope(&log_s, &log_f);
    clamp_01(slope)
}

/// RMS of residuals after fitting a least-squares line to the segment.
///
/// For evenly-spaced x = 0, 1, ..., s-1 the closed-form slope/intercept avoids
/// building a full matrix.
fn box_rms(segment: &[f32]) -> f32 {
    let s = segment.len();
    if s < 2 {
        return 0.0;
    }
    let sf = s as f64;

    // Sums for least-squares (using f64 accumulators to limit rounding).
    let sum_x: f64 = (sf - 1.0) * sf / 2.0;
    let sum_x2: f64 = (sf - 1.0) * sf * (2.0 * sf - 1.0) / 6.0;

    let mut sum_y: f64 = 0.0;
    let mut sum_xy: f64 = 0.0;
    for (i, &y) in segment.iter().enumerate() {
        let yd = y as f64;
        sum_y += yd;
        sum_xy += i as f64 * yd;
    }

    let denom = sf * sum_x2 - sum_x * sum_x;
    if denom.abs() < 1e-30 {
        return 0.0;
    }
    let slope = (sf * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / sf;

    // RMS of residuals.
    let mut ss = 0.0_f64;
    for (i, &y) in segment.iter().enumerate() {
        let fit = intercept + slope * i as f64;
        let resid = y as f64 - fit;
        ss += resid * resid;
    }

    (ss / sf).sqrt() as f32
}

/// Simple two-variable OLS slope: slope = cov(x,y) / var(x).
fn least_squares_slope(x: &[f32], y: &[f32]) -> f32 {
    let n = x.len() as f64;
    let mean_x = x.iter().copied().map(|v| v as f64).sum::<f64>() / n;
    let mean_y = y.iter().copied().map(|v| v as f64).sum::<f64>() / n;

    let mut cov = 0.0_f64;
    let mut var = 0.0_f64;
    for (&xi, &yi) in x.iter().zip(y.iter()) {
        let dx = xi as f64 - mean_x;
        let dy = yi as f64 - mean_y;
        cov += dx * dy;
        var += dx * dx;
    }

    if var.abs() < 1e-30 {
        return 0.5;
    }
    (cov / var) as f32
}

/// Clamp a value to [0, 1], mapping NaN to 0.5.
fn clamp_01(v: f32) -> f32 {
    if v.is_nan() {
        return 0.5;
    }
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_constant_price_returns_neutral() {
        // Constant price => zero returns => H should be ~0.5 (degenerate).
        let close = vec![100.0_f32; 64];
        let h = compute_hurst(&close, 64);
        // Degenerate input: all returns are zero, profile is zero,
        // box_rms is 0 => log(0) is -inf => no valid points => 0.5 fallback.
        assert!((h - 0.5).abs() < 0.01, "got {h}");
    }

    #[test]
    fn too_short_returns_default() {
        let close = vec![1.0, 2.0, 3.0];
        assert_eq!(compute_hurst(&close, 500), 0.5);
    }

    #[test]
    fn pure_trend_returns_high_hurst() {
        // Monotonically increasing prices should have H > 0.5.
        let close: Vec<f32> = (0..600).map(|i| 100.0 + i as f32 * 0.1).collect();
        let h = compute_hurst(&close, 500);
        assert!(h > 0.5, "trending data got H={h}");
    }
}
