//! 1.6 Wavelet Scattering Transform -- shift-invariant feature extraction.
//!
//! Computes order-1 and order-2 scattering coefficients using Morlet
//! wavelets at `j_max` dyadic scales, then applies a pre-trained linear
//! classifier (sigmoid of dot product) to produce a breakout probability.
//!
//! All convolutions are performed in the frequency domain via `rustfft`.

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;

/// Compute the scattering classification score.
///
/// # Arguments
/// * `window` -- close prices; length should ideally be a power of 2 (64 or 128).
/// * `j_max` -- number of octave scales (e.g. 6).
/// * `weights` -- pre-trained classifier weights, length = number of scattering coefficients.
/// * `bias` -- classifier bias.
/// * `fft_scratch` -- reusable buffer.
///
/// # Returns
/// Classification score in (0, 1) where 1 = pre-breakout.
#[allow(clippy::needless_range_loop)]
pub fn compute_scattering(
    window: &[f32],
    j_max: usize,
    weights: &[f32],
    bias: f32,
    fft_scratch: &mut Vec<Complex32>,
) -> f32 {
    let n = window.len();
    if n < 4 || j_max == 0 {
        return 0.5; // neutral
    }

    let fft_len = n.next_power_of_two();
    ensure_scratch(fft_scratch, fft_len);

    let mut planner = FftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(fft_len);
    let inv = planner.plan_fft_inverse(fft_len);

    // FFT of the input signal.
    let mut x_fft = vec![Complex32::new(0.0, 0.0); fft_len];
    for (i, &v) in window.iter().enumerate() {
        x_fft[i] = Complex32::new(v, 0.0);
    }
    fwd.process(&mut x_fft);

    // Precompute Morlet wavelet spectra at J scales.
    let wavelets: Vec<Vec<Complex32>> = (0..j_max)
        .map(|j| morlet_spectrum(fft_len, j))
        .collect();

    // Order 1 coefficients: S1[j] = mean(|x * psi_j|).
    let mut s1_responses: Vec<Vec<f32>> = Vec::with_capacity(j_max);
    let mut coeffs: Vec<f32> = Vec::new();

    for j in 0..j_max {
        let modulus = convolve_modulus(&x_fft, &wavelets[j], fft_len, &inv);
        let s1 = mean_abs(&modulus, n);
        coeffs.push(s1);
        s1_responses.push(modulus);
    }

    // Order 2 coefficients: S2[j1, j2] = mean(||x * psi_j1| * psi_j2|) for j2 > j1.
    for j1 in 0..j_max {
        // FFT of the order-1 modulus.
        let mut u1_fft = vec![Complex32::new(0.0, 0.0); fft_len];
        for (i, &v) in s1_responses[j1].iter().enumerate().take(fft_len) {
            u1_fft[i] = Complex32::new(v, 0.0);
        }
        fwd.process(&mut u1_fft);

        for j2 in (j1 + 1)..j_max {
            let modulus = convolve_modulus(&u1_fft, &wavelets[j2], fft_len, &inv);
            let s2 = mean_abs(&modulus, n);
            coeffs.push(s2);
        }
    }

    // Apply classifier: sigmoid(dot(coeffs, weights) + bias).
    if weights.is_empty() {
        // No trained weights -- return a heuristic: normalised energy concentration.
        let total: f32 = coeffs.iter().sum::<f32>().max(1e-10);
        let top = coeffs.iter().cloned().fold(0.0_f32, f32::max);
        return sigmoid(top / total);
    }

    let dot: f32 = coeffs
        .iter()
        .zip(weights.iter())
        .map(|(c, w)| c * w)
        .sum();
    sigmoid(dot + bias)
}

/// Morlet wavelet spectrum at scale j (dyadic: center freq = pi / 2^j).
fn morlet_spectrum(fft_len: usize, j: usize) -> Vec<Complex32> {
    let omega0 = std::f32::consts::PI / (1 << j) as f32;
    let sigma = 1.0 / (omega0.max(0.01));
    (0..fft_len)
        .map(|i| {
            let omega = 2.0 * std::f32::consts::PI * i as f32 / fft_len as f32;
            let diff = omega - omega0;
            let val = (-0.5 * diff * diff * sigma * sigma).exp();
            Complex32::new(val, 0.0)
        })
        .collect()
}

/// Convolve (in frequency domain) and take modulus (time domain).
fn convolve_modulus(
    x_fft: &[Complex32],
    psi_fft: &[Complex32],
    fft_len: usize,
    inv: &std::sync::Arc<dyn rustfft::Fft<f32>>,
) -> Vec<f32> {
    let mut product: Vec<Complex32> = x_fft
        .iter()
        .zip(psi_fft.iter())
        .map(|(a, b)| a * b)
        .collect();
    product.resize(fft_len, Complex32::new(0.0, 0.0));
    inv.process(&mut product);
    let inv_n = 1.0 / fft_len as f32;
    product.iter().map(|c| (c.re * inv_n).abs()).collect()
}

/// Mean of the first `n` absolute values.
fn mean_abs(data: &[f32], n: usize) -> f32 {
    let len = data.len().min(n);
    if len == 0 {
        return 0.0;
    }
    data[..len].iter().map(|v| v.abs()).sum::<f32>() / len as f32
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x.clamp(-88.0, 88.0)).exp())
}

fn ensure_scratch(scratch: &mut Vec<Complex32>, len: usize) {
    if scratch.len() < len {
        scratch.resize(len, Complex32::new(0.0, 0.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_valid_score() {
        let window: Vec<f32> = (0..64).map(|i| 100.0 + (i as f32 * 0.1).sin()).collect();
        let mut scratch = Vec::new();
        let score = compute_scattering(&window, 4, &[], 0.0, &mut scratch);
        assert!(score > 0.0 && score < 1.0, "score = {score}");
    }

    #[test]
    fn short_window_returns_neutral() {
        let mut scratch = Vec::new();
        let score = compute_scattering(&[1.0, 2.0], 6, &[], 0.0, &mut scratch);
        assert!((score - 0.5).abs() < 0.01);
    }
}
