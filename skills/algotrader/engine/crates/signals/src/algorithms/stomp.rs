//! 1.8 STOMP Matrix Profile -- FFT-accelerated subsequence similarity search.
//!
//! Computes the minimum z-normalised Euclidean distance between a query
//! subsequence and all subsequences of a reference time series.  Low distance
//! means the current price action closely resembles a historical motif; high
//! distance means novelty / discord.
//!
//! The distance computation uses the MASS (Mueen's Algorithm for Similarity
//! Search) approach: sliding dot product via FFT, then convert to
//! z-normalised distance.

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;

/// Compute the matrix-profile nearest-neighbour distance for a query against
/// a reference time series.
///
/// # Arguments
/// * `ts` -- full reference time series (length T).
/// * `query` -- current subsequence (length m).
/// * `m` -- subsequence length (must equal `query.len()`).
/// * `fft_scratch` -- reusable scratch buffer.
///
/// # Returns
/// Minimum z-normalised Euclidean distance to any subsequence in `ts`.
/// Returns `f32::MAX` if inputs are degenerate.
pub fn compute_stomp(
    ts: &[f32],
    query: &[f32],
    m: usize,
    fft_scratch: &mut Vec<Complex32>,
) -> f32 {
    if m < 2 || query.len() < m || ts.len() < m {
        return f32::MAX;
    }

    let n = ts.len();
    let n_subs = n - m + 1;
    let fft_len = (n + m).next_power_of_two();
    ensure_scratch(fft_scratch, fft_len);

    // Z-normalise the query.
    let (q_mean, q_std) = mean_std(query);
    if q_std < 1e-10 {
        return f32::MAX; // constant query -- undefined distance
    }
    let q_norm: Vec<f32> = query.iter().map(|&v| (v - q_mean) / q_std).collect();

    // Precompute rolling mean and std of ts subsequences.
    let (_ts_means, ts_stds) = rolling_mean_std(ts, m);

    // Sliding dot product via FFT (MASS).
    let qdf = mass_dot_product(ts, &q_norm, fft_len, fft_scratch);

    // Convert to z-normalised Euclidean distance.
    let mut min_dist = f32::MAX;
    for i in 0..n_subs {
        let std_i = ts_stds[i];
        if std_i < 1e-10 {
            continue; // skip constant subsequences
        }
        // dist^2 = 2m * (1 - (QT_i - m * mean_i * 0) / (m * std_i))
        // Since q_norm has zero mean: QT_i = dot(ts_sub_i, q_norm)
        // dist^2 = 2m * (1 - QT_i / (m * std_i))
        let qt = if i < qdf.len() { qdf[i] } else { 0.0 };
        let pearson = qt / (m as f32 * std_i);
        let dist_sq = (2.0 * m as f32 * (1.0 - pearson.clamp(-1.0, 1.0))).max(0.0);
        let dist = dist_sq.sqrt();
        if dist < min_dist {
            min_dist = dist;
        }
    }

    min_dist
}

/// Compute the sliding dot product between `ts` and `q_norm` using FFT.
/// Returns a vector of length (ts.len() - q_norm.len() + 1).
fn mass_dot_product(
    ts: &[f32],
    q_norm: &[f32],
    fft_len: usize,
    _scratch: &mut Vec<Complex32>,
) -> Vec<f32> {
    let mut planner = FftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(fft_len);
    let inv = planner.plan_fft_inverse(fft_len);

    // FFT of ts (zero-padded).
    let mut ts_fft = vec![Complex32::new(0.0, 0.0); fft_len];
    for (i, &v) in ts.iter().enumerate() {
        ts_fft[i] = Complex32::new(v, 0.0);
    }
    fwd.process(&mut ts_fft);

    // FFT of reversed query (zero-padded).
    let mut q_fft = vec![Complex32::new(0.0, 0.0); fft_len];
    for (i, &v) in q_norm.iter().rev().enumerate() {
        q_fft[i] = Complex32::new(v, 0.0);
    }
    fwd.process(&mut q_fft);

    // Multiply and IFFT.
    let mut product: Vec<Complex32> = ts_fft
        .iter()
        .zip(q_fft.iter())
        .map(|(a, b)| a * b)
        .collect();
    inv.process(&mut product);

    let inv_n = 1.0 / fft_len as f32;
    let m = q_norm.len();
    let n_subs = ts.len() - m + 1;

    // The correlation peaks are at indices (m-1) .. (m-1 + n_subs - 1).
    let offset = m - 1;
    (0..n_subs)
        .map(|i| {
            if offset + i < product.len() {
                product[offset + i].re * inv_n
            } else {
                0.0
            }
        })
        .collect()
}

/// Rolling mean and standard deviation for all subsequences of length m.
fn rolling_mean_std(ts: &[f32], m: usize) -> (Vec<f32>, Vec<f32>) {
    let n = ts.len();
    let n_subs = n - m + 1;
    let mut means = vec![0.0_f32; n_subs];
    let mut stds = vec![0.0_f32; n_subs];

    // Cumulative sums for O(n) rolling stats.
    let mut cum_sum = vec![0.0_f32; n + 1];
    let mut cum_sq = vec![0.0_f32; n + 1];
    for i in 0..n {
        cum_sum[i + 1] = cum_sum[i] + ts[i];
        cum_sq[i + 1] = cum_sq[i] + ts[i] * ts[i];
    }

    let mf = m as f32;
    for i in 0..n_subs {
        let s = cum_sum[i + m] - cum_sum[i];
        let sq = cum_sq[i + m] - cum_sq[i];
        let mean = s / mf;
        let var = (sq / mf - mean * mean).max(0.0);
        means[i] = mean;
        stds[i] = var.sqrt();
    }

    (means, stds)
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
    fn self_match_gives_near_zero() {
        let ts: Vec<f32> = (0..100).map(|i| (i as f32 * 0.1).sin() * 10.0 + 100.0).collect();
        let query = &ts[20..50]; // exact subsequence from ts
        let mut scratch = Vec::new();
        let dist = compute_stomp(&ts, query, 30, &mut scratch);
        assert!(dist < 0.5, "self-match distance should be near zero, got {dist}");
    }

    #[test]
    fn short_inputs_return_max() {
        let mut scratch = Vec::new();
        assert_eq!(compute_stomp(&[1.0], &[1.0], 5, &mut scratch), f32::MAX);
    }
}
