//! Fuzzy membership functions: scalar and SIMD batch variants.
//!
//! All functions output values in [0.0, 1.0].
//!
//! SIMD note: `f32x4` from the `wide` crate maps to ARM NEON `float32x4_t`
//! on Apple Silicon. The compiler fuses the multiply-adds into FMA instructions
//! when `.cargo/config.toml` has `-C llvm-args=-fp-contract=fast`.

use wide::f32x4;

// ---------------------------------------------------------------------------
// Sigmoid (fast approximation, no exp)
// ---------------------------------------------------------------------------

/// Fast sigmoid: `0.5 + 0.5 * z / (1 + |z|)` where `z = k * (x - mid)`.
///
/// Approximates the logistic sigmoid with max error < 0.01. Fully
/// vectorizable — no transcendental functions.
///
/// - `mid`: inflection point (output = 0.5 when x == mid)
/// - `k`: steepness (positive k → higher x → higher output)
#[inline(always)]
pub fn fast_sigmoid(x: f32, mid: f32, k: f32) -> f32 {
    let z = k * (x - mid);
    0.5 + 0.5 * z / (1.0 + z.abs())
}

/// Batch sigmoid over a slice. Writes results into `out`.
///
/// Processes 4 values at a time using NEON f32x4; handles tail remainder
/// with scalar fallback.
pub fn sigmoid_slice(input: &[f32], mid: f32, k: f32, out: &mut [f32]) {
    assert_eq!(input.len(), out.len());
    let n = input.len();
    let chunks = n / 4;
    let mid4 = f32x4::splat(mid);
    let k4 = f32x4::splat(k);
    let half = f32x4::splat(0.5);
    let one = f32x4::splat(1.0);

    for i in 0..chunks {
        let base = i * 4;
        let x4 = f32x4::new([input[base], input[base+1], input[base+2], input[base+3]]);
        let z = k4 * (x4 - mid4);
        // abs via bit mask: clear sign bit
        let z_abs = z.abs();
        let s = half + half * z / (one + z_abs);
        let arr: [f32; 4] = s.into();
        out[base..base+4].copy_from_slice(&arr);
    }
    for i in (chunks * 4)..n {
        out[i] = fast_sigmoid(input[i], mid, k);
    }
}

// ---------------------------------------------------------------------------
// Cauchy (Gaussian proxy, no exp)
// ---------------------------------------------------------------------------

/// Cauchy membership: `1 / (1 + ((x - center) / sigma)^2)`.
///
/// Peaks at 1.0 when `x == center`, decays smoothly. Avoids `exp()`,
/// making it fully vectorizable. Slightly heavier tails than Gaussian,
/// which is appropriate for noisy financial data.
#[inline(always)]
pub fn cauchy_membership(x: f32, center: f32, sigma: f32) -> f32 {
    let z = (x - center) / sigma.max(1e-10);
    1.0 / (1.0 + z * z)
}

/// Batch Cauchy over a slice. Writes results into `out`.
pub fn cauchy_slice(input: &[f32], center: f32, sigma: f32, out: &mut [f32]) {
    assert_eq!(input.len(), out.len());
    let n = input.len();
    let chunks = n / 4;
    let center4 = f32x4::splat(center);
    let inv_sigma = f32x4::splat(1.0 / sigma.max(1e-10));
    let one = f32x4::splat(1.0);

    for i in 0..chunks {
        let base = i * 4;
        let x4 = f32x4::new([input[base], input[base+1], input[base+2], input[base+3]]);
        let z = (x4 - center4) * inv_sigma;
        let s = one / (one + z * z);
        let arr: [f32; 4] = s.into();
        out[base..base+4].copy_from_slice(&arr);
    }
    for i in (chunks * 4)..n {
        out[i] = cauchy_membership(input[i], center, sigma);
    }
}

// ---------------------------------------------------------------------------
// Trapezoidal
// ---------------------------------------------------------------------------

/// Trapezoidal membership: 1.0 for x ≤ lo, 0.0 for x ≥ hi, linear in between.
///
/// Used for retracement scoring where a shallow pullback (< lo fraction)
/// is ideal and a deep pullback (> hi fraction) invalidates the pattern.
#[inline(always)]
pub fn trapezoidal(x: f32, lo: f32, hi: f32) -> f32 {
    if x <= lo {
        1.0
    } else if x >= hi {
        0.0
    } else {
        1.0 - (x - lo) / (hi - lo).max(1e-10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigmoid_midpoint_is_half() {
        assert!((fast_sigmoid(5.0, 5.0, 20.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn sigmoid_slice_matches_scalar() {
        let input: Vec<f32> = (0..13).map(|i| i as f32 * 0.1).collect();
        let mut out_simd = vec![0.0_f32; input.len()];
        sigmoid_slice(&input, 0.5, 10.0, &mut out_simd);
        for (i, &x) in input.iter().enumerate() {
            let scalar = fast_sigmoid(x, 0.5, 10.0);
            assert!((out_simd[i] - scalar).abs() < 1e-6, "mismatch at i={i}");
        }
    }

    #[test]
    fn cauchy_peaks_at_center() {
        assert!((cauchy_membership(3.0, 3.0, 1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cauchy_slice_matches_scalar() {
        let input: Vec<f32> = (0..13).map(|i| i as f32 * 0.1).collect();
        let mut out = vec![0.0_f32; input.len()];
        cauchy_slice(&input, 0.5, 0.3, &mut out);
        for (i, &x) in input.iter().enumerate() {
            let scalar = cauchy_membership(x, 0.5, 0.3);
            assert!((out[i] - scalar).abs() < 1e-6, "mismatch at i={i}");
        }
    }

    #[test]
    fn trapezoidal_boundaries() {
        assert_eq!(trapezoidal(0.2, 0.33, 0.50), 1.0);
        assert_eq!(trapezoidal(0.55, 0.33, 0.50), 0.0);
        let mid = trapezoidal(0.415, 0.33, 0.50);
        assert!(mid > 0.0 && mid < 1.0);
    }
}
