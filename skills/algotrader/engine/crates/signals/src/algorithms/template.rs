//! 1.9 Template Matching -- LIGO-style cross-correlation against a bank of
//! known consolidation shapes.
//!
//! Z-normalises the current window and each template, then computes the
//! maximum cross-correlation via FFT.  The best match across all templates
//! is returned.  High correlation (close to 1.0) means the current price
//! action closely resembles a known pre-breakout pattern.

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;

/// Compute the maximum template-match correlation.
///
/// # Arguments
/// * `window` -- current close price window (length W).
/// * `templates` -- pre-computed template bank (each template z-normalised,
///   same length as `window` or shorter).
/// * `fft_scratch` -- reusable buffer.
///
/// # Returns
/// Maximum Pearson correlation in [-1, 1].  Returns 0.0 on degenerate inputs.
pub fn compute_template_match(
    window: &[f32],
    templates: &[&[f32]],
    fft_scratch: &mut Vec<Complex32>,
) -> f32 {
    if window.len() < 2 || templates.is_empty() {
        return 0.0;
    }

    // Z-normalise the window.
    let (w_mean, w_std) = mean_std(window);
    if w_std < 1e-10 {
        return 0.0; // constant window
    }
    let w_norm: Vec<f32> = window.iter().map(|&v| (v - w_mean) / w_std).collect();

    let mut best_corr = -1.0_f32;

    for &tmpl in templates {
        if tmpl.is_empty() {
            continue;
        }

        // Z-normalise the template.
        let (t_mean, t_std) = mean_std(tmpl);
        if t_std < 1e-10 {
            continue;
        }
        let t_norm: Vec<f32> = tmpl.iter().map(|&v| (v - t_mean) / t_std).collect();

        let corr = max_cross_correlation(&w_norm, &t_norm, fft_scratch);
        if corr > best_corr {
            best_corr = corr;
        }
    }

    if best_corr < -1.0 {
        return 0.0;
    }
    best_corr.clamp(-1.0, 1.0)
}

/// FFT cross-correlation, returning the maximum normalised correlation.
fn max_cross_correlation(
    a: &[f32],
    b: &[f32],
    scratch: &mut Vec<Complex32>,
) -> f32 {
    let fft_len = (a.len() + b.len()).next_power_of_two();
    ensure_scratch(scratch, fft_len);

    let mut planner = FftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(fft_len);
    let inv = planner.plan_fft_inverse(fft_len);

    // FFT of a (zero-padded).
    let mut a_fft = vec![Complex32::new(0.0, 0.0); fft_len];
    for (i, &v) in a.iter().enumerate() {
        a_fft[i] = Complex32::new(v, 0.0);
    }
    fwd.process(&mut a_fft);

    // FFT of b reversed (zero-padded) -- convolution = correlation with reversal.
    let mut b_fft = vec![Complex32::new(0.0, 0.0); fft_len];
    for (i, &v) in b.iter().rev().enumerate() {
        b_fft[i] = Complex32::new(v, 0.0);
    }
    fwd.process(&mut b_fft);

    // Multiply and IFFT.
    let mut product: Vec<Complex32> = a_fft
        .iter()
        .zip(b_fft.iter())
        .map(|(x, y)| x * y)
        .collect();
    inv.process(&mut product);

    let inv_n = 1.0 / fft_len as f32;
    // Normalise by the geometric mean of energy (both are z-normalised so
    // energy = length).
    let norm = (a.len().min(b.len()) as f32).max(1.0);

    let mut max_corr = f32::NEG_INFINITY;
    for c in &product {
        let val = c.re * inv_n / norm;
        if val > max_corr {
            max_corr = val;
        }
    }

    max_corr
}

fn mean_std(data: &[f32]) -> (f32, f32) {
    let n = data.len() as f32;
    if n == 0.0 {
        return (0.0, 0.0);
    }
    let mean = data.iter().sum::<f32>() / n;
    let var = data.iter().map(|&v| (v - mean) * (v - mean)).sum::<f32>() / n;
    (mean, var.sqrt())
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
    fn exact_template_match_gives_high_correlation() {
        let window: Vec<f32> = (0..50).map(|i| (i as f32 * 0.2).sin()).collect();
        let template = window.clone();
        let templates: Vec<&[f32]> = vec![template.as_slice()];
        let mut scratch = Vec::new();
        let corr = compute_template_match(&window, &templates, &mut scratch);
        assert!(corr > 0.9, "exact match should give high correlation, got {corr}");
    }

    #[test]
    fn empty_templates_returns_zero() {
        let window = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mut scratch = Vec::new();
        let corr = compute_template_match(&window, &[], &mut scratch);
        assert_eq!(corr, 0.0);
    }
}
