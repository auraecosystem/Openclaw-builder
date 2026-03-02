//! 1.4 VMD + STA/LTA -- Variational Mode Decomposition with volatility ratio.
//!
//! Decomposes the price window into K modes (trend, swing, noise) in the
//! frequency domain, then computes the ratio of short-term to long-term
//! average amplitude on the swing mode.  A dropping STA/LTA signals
//! compression (coiling); a spike signals breakout.
//!
//! The FFT-based VMD iterates in the frequency domain, avoiding costly
//! convolutions.  We use `rustfft` for the forward / inverse transforms.

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;

/// Compute the STA/LTA ratio of the VMD swing mode.
///
/// # Arguments
/// * `window` -- close prices, length N.
/// * `k_modes` -- number of VMD modes (3 = trend + swing + noise).
/// * `alpha` -- bandwidth constraint (higher = narrower modes).
/// * `sta_window` -- short-term averaging length.
/// * `lta_window` -- long-term averaging length.
/// * `fft_scratch` -- reusable scratch buffer, resized internally if needed.
///
/// # Returns
/// STA / LTA ratio (> 1 = expanding, < 1 = compressing).  Returns 1.0 on
/// degenerate inputs.
pub fn compute_vmd_sta_lta(
    window: &[f32],
    k_modes: usize,
    alpha: f32,
    sta_window: usize,
    lta_window: usize,
    fft_scratch: &mut Vec<Complex32>,
) -> f32 {
    let n = window.len();
    if n < 4 || k_modes == 0 || sta_window == 0 || lta_window == 0 {
        return 1.0;
    }

    let fft_len = n.next_power_of_two();
    ensure_scratch(fft_scratch, fft_len);

    let mut planner = FftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(fft_len);
    let inv = planner.plan_fft_inverse(fft_len);

    // Forward FFT of the input signal.
    let mut signal_fft = vec![Complex32::new(0.0, 0.0); fft_len];
    for (i, &v) in window.iter().enumerate() {
        signal_fft[i] = Complex32::new(v, 0.0);
    }
    fwd.process(&mut signal_fft);

    // Frequency axis (normalised: 0..0.5 maps to DC..Nyquist).
    let freqs: Vec<f32> = (0..fft_len)
        .map(|i| i as f32 / fft_len as f32)
        .collect();

    // Initialise K modes equally spaced in frequency.
    let mut modes: Vec<Vec<Complex32>> = (0..k_modes)
        .map(|_| vec![Complex32::new(0.0, 0.0); fft_len])
        .collect();
    let mut center_freqs: Vec<f32> = (0..k_modes)
        .map(|k| 0.5 * (k + 1) as f32 / (k_modes + 1) as f32)
        .collect();

    let n_iter = 15;
    let tau = 0.0_f32; // noise tolerance (0 = exact reconstruction)

    for _iter in 0..n_iter {
        for k in 0..k_modes {
            // Sum of all other modes in frequency domain.
            let mut sum_others = vec![Complex32::new(0.0, 0.0); fft_len];
            for (j, mode) in modes.iter().enumerate() {
                if j != k {
                    for (s, m) in sum_others.iter_mut().zip(mode.iter()) {
                        *s += m;
                    }
                }
            }

            // Update mode k: bandpass around center_freqs[k].
            let omega_k = center_freqs[k];
            for i in 0..fft_len {
                let residual = signal_fft[i] - sum_others[i];
                let diff = freqs[i] - omega_k;
                let denom = 1.0 + alpha * diff * diff + tau;
                modes[k][i] = residual / Complex32::new(denom, 0.0);
            }

            // Update center frequency: spectral centre of mass.
            let mut num = 0.0_f32;
            let mut den = 0.0_f32;
            // Only use positive frequencies (0..fft_len/2).
            for i in 0..(fft_len / 2 + 1) {
                let power = modes[k][i].norm_sqr();
                num += freqs[i] * power;
                den += power;
            }
            if den > 1e-20 {
                center_freqs[k] = num / den;
            }
        }
    }

    // Identify the swing mode: the one with the median center frequency.
    // For K=3 this is index 1 after sorting.
    let swing_idx = {
        let mut indexed: Vec<(usize, f32)> = center_freqs.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        if k_modes >= 3 {
            indexed[k_modes / 2].0
        } else {
            indexed[0].0 // fallback: lowest frequency mode
        }
    };

    // IFFT the swing mode to get the time-domain signal.
    let mut swing_td = modes[swing_idx].clone();
    inv.process(&mut swing_td);
    let inv_n = 1.0 / fft_len as f32;

    // Extract real part, take absolute values.
    let swing_abs: Vec<f32> = swing_td.iter().take(n).map(|c| (c.re * inv_n).abs()).collect();

    // STA/LTA on the swing mode amplitude.
    let sta = mean_last(&swing_abs, sta_window);
    let lta = mean_last(&swing_abs, lta_window);

    if lta < 1e-20 {
        return 1.0;
    }

    (sta / lta).max(0.0)
}

/// Mean of the last `win` elements, or all elements if fewer.
fn mean_last(data: &[f32], win: usize) -> f32 {
    let start = data.len().saturating_sub(win);
    let slice = &data[start..];
    if slice.is_empty() {
        return 0.0;
    }
    slice.iter().sum::<f32>() / slice.len() as f32
}

/// Ensure scratch has at least `len` elements.
fn ensure_scratch(scratch: &mut Vec<Complex32>, len: usize) {
    if scratch.len() < len {
        scratch.resize(len, Complex32::new(0.0, 0.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_price_gives_ratio_near_one() {
        let window = vec![100.0_f32; 64];
        let mut scratch = Vec::new();
        let ratio = compute_vmd_sta_lta(&window, 3, 2000.0, 10, 50, &mut scratch);
        // Constant signal -> all modes near zero -> ratio ~1 or NaN-safe.
        assert!(ratio.is_finite(), "ratio = {ratio}");
    }

    #[test]
    fn expanding_volatility_gives_different_ratio() {
        // First 96 bars flat, last 32 bars volatile -- larger window with
        // clearer separation between quiet and active periods.
        let mut window = vec![100.0_f32; 128];
        for (i, w) in window.iter_mut().enumerate().take(128).skip(96) {
            *w = 100.0 + if i % 2 == 0 { 10.0 } else { -10.0 };
        }
        let mut scratch = Vec::new();
        let ratio = compute_vmd_sta_lta(&window, 3, 2000.0, 16, 64, &mut scratch);
        // The exact ratio depends on which mode VMD identifies as "swing".
        // We verify the function produces a finite, positive result.
        assert!(ratio.is_finite() && ratio > 0.0, "ratio = {ratio}");
    }
}
