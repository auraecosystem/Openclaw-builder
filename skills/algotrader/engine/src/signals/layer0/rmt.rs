//! 0.3 RMT Correlation Filtering
//!
//! Applies Marchenko-Pastur noise filtering to the empirical correlation
//! matrix of multi-asset returns. Uses `faer` for eigendecomposition.

use faer::{Mat, Side};

/// Result of the RMT-filtered correlation analysis.
#[derive(Clone, Debug)]
pub struct RmtResult {
    /// Filtered correlation matrix, row-major `n_tickers * n_tickers`.
    pub filtered_corr: Vec<f32>,
    /// Eigenvalues that exceeded the Marchenko-Pastur upper bound (signal).
    pub top_eigenvalues: Vec<f32>,
    /// Number of significant factors (eigenvalues above MP upper bound).
    pub n_factors: usize,
}

/// Compute the RMT-filtered correlation matrix from a flat row-major returns
/// matrix of shape `(n_tickers, n_bars)`.
///
/// Algorithm:
/// 1. Standardize each ticker's returns (zero mean, unit variance).
/// 2. Compute the empirical correlation matrix C = (1/T) * X * X^T.
/// 3. Eigendecompose C using faer's self-adjoint solver.
/// 4. Apply Marchenko-Pastur bounds: lambda_+/- = (1 +/- sqrt(N/T))^2.
///    Eigenvalues within bounds are noise; replace with their mean.
/// 5. Reconstruct the filtered correlation matrix.
///
/// Returns `None` (via early return of a degenerate `RmtResult`) when input
/// dimensions are too small or contain NaN.
pub fn compute_rmt_filtered(
    returns: &[f32],
    n_tickers: usize,
    n_bars: usize,
) -> RmtResult {
    let empty = RmtResult {
        filtered_corr: Vec::new(),
        top_eigenvalues: Vec::new(),
        n_factors: 0,
    };

    // Validate dimensions.
    if n_tickers < 2 || n_bars < 2 || returns.len() < n_tickers * n_bars {
        return empty;
    }

    let n = n_tickers;
    let t = n_bars;

    // --- 1. Standardize each ticker's returns ---
    // Build a faer Mat<f32> of shape (n, t) with standardized rows.
    let x = match standardize_rows(returns, n, t) {
        Some(m) => m,
        None => return empty,
    };

    // --- 2. Empirical correlation matrix C = (1/T) * X * X^T ---
    // faer: X is (n, t). C = X * X^T is (n, n). We scale by 1/t.
    let c_full = &x * x.transpose();
    let inv_t = 1.0_f32 / t as f32;
    let c = Mat::<f32>::from_fn(n, n, |i, j| c_full.read(i, j) * inv_t);

    // --- 3. Eigendecompose (self-adjoint, lower triangle) ---
    let evd = c.selfadjoint_eigendecomposition(Side::Lower);
    let eigenvalues_col = evd.s().column_vector();
    let eigenvectors = evd.u(); // (n, n) matrix whose columns are eigenvectors

    let eigenvalues: Vec<f32> = (0..n).map(|i| eigenvalues_col.read(i)).collect();

    // Guard against NaN from degenerate matrices.
    if eigenvalues.iter().any(|v| v.is_nan()) {
        return empty;
    }

    // --- 4. Marchenko-Pastur filtering ---
    let q = n as f32 / t as f32; // ratio N/T
    let sqrt_q = q.sqrt();
    let lambda_plus = (1.0 + sqrt_q) * (1.0 + sqrt_q);
    let lambda_minus = (1.0 - sqrt_q) * (1.0 - sqrt_q);

    // Partition eigenvalues into noise vs signal.
    let mut noise_sum = 0.0_f32;
    let mut noise_count = 0_usize;
    let mut signal_indices = Vec::new();

    for (i, &lam) in eigenvalues.iter().enumerate() {
        if lam >= lambda_minus && lam <= lambda_plus {
            noise_sum += lam;
            noise_count += 1;
        } else if lam > lambda_plus {
            signal_indices.push(i);
        }
        // eigenvalues below lambda_minus are also noise (shouldn't happen
        // often for a valid correlation matrix but handle gracefully).
        if lam < lambda_minus {
            noise_sum += lam;
            noise_count += 1;
        }
    }

    let noise_replacement = if noise_count > 0 {
        noise_sum / noise_count as f32
    } else {
        1.0 // fallback: if all eigenvalues are signal, use 1.0
    };

    // Replace noise eigenvalues with their average.
    let mut filtered_eig = eigenvalues.clone();
    for (i, lam) in filtered_eig.iter_mut().enumerate() {
        if !signal_indices.contains(&i) {
            *lam = noise_replacement;
        }
    }

    // Collect top eigenvalues (signal only).
    let top_eigenvalues: Vec<f32> = signal_indices.iter().map(|&i| eigenvalues[i]).collect();
    let n_factors = signal_indices.len();

    // --- 5. Reconstruct filtered correlation matrix ---
    // C_filtered = U * diag(filtered_eig) * U^T
    // Then force diagonal to 1.0 (correlation normalization).
    let mut filtered = vec![0.0_f32; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut val = 0.0_f32;
            #[allow(clippy::needless_range_loop)]
            for k in 0..n {
                val += eigenvectors.read(i, k) * filtered_eig[k] * eigenvectors.read(j, k);
            }
            filtered[i * n + j] = val;
        }
    }

    // Normalize to correlation: C_ij / sqrt(C_ii * C_jj), force diagonal = 1.
    let diag_vals: Vec<f32> = (0..n).map(|i| filtered[i * n + i]).collect();
    for i in 0..n {
        for j in 0..n {
            if i == j {
                filtered[i * n + j] = 1.0;
            } else {
                let denom = (diag_vals[i] * diag_vals[j]).sqrt();
                if denom > 1e-12 {
                    filtered[i * n + j] /= denom;
                    // Clamp to [-1, 1].
                    filtered[i * n + j] = filtered[i * n + j].clamp(-1.0, 1.0);
                } else {
                    filtered[i * n + j] = 0.0;
                }
            }
        }
    }

    RmtResult {
        filtered_corr: filtered,
        top_eigenvalues,
        n_factors,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Standardize each row of the (n_tickers, n_bars) matrix to zero mean and
/// unit variance. Returns a faer Mat<f32> of the same shape, or None if any
/// ticker has zero or NaN variance.
fn standardize_rows(returns: &[f32], n: usize, t: usize) -> Option<Mat<f32>> {
    // Compute per-ticker mean and std.
    let mut means = Vec::with_capacity(n);
    let mut stds = Vec::with_capacity(n);

    for ticker in 0..n {
        let row = &returns[ticker * t..(ticker + 1) * t];

        let sum: f64 = row.iter().map(|&v| v as f64).sum();
        let mean = sum / t as f64;
        if mean.is_nan() {
            return None;
        }

        let var: f64 = row.iter().map(|&v| {
            let d = v as f64 - mean;
            d * d
        }).sum::<f64>() / t as f64;

        let std = var.sqrt();
        if std < 1e-12 || std.is_nan() {
            return None;
        }

        means.push(mean);
        stds.push(std);
    }

    let mat = Mat::<f32>::from_fn(n, t, |i, j| {
        let val = returns[i * t + j] as f64;
        ((val - means[i]) / stds[i]) as f32
    });

    Some(mat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn too_few_tickers() {
        let result = compute_rmt_filtered(&[0.0; 10], 1, 10);
        assert!(result.filtered_corr.is_empty());
    }

    #[test]
    fn identity_correlation_for_independent_tickers() {
        // Two independent tickers with unit-variance noise. The filtered
        // correlation off-diag should be small.
        let t = 500;
        let n = 2;
        let mut data = vec![0.0_f32; n * t];

        // Deterministic pseudo-random: simple LCG.
        let mut seed = 42u64;
        let lcg = |s: &mut u64| -> f32 {
            *s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((*s >> 33) as f32 / (1u64 << 31) as f32) - 1.0
        };

        for v in data.iter_mut() {
            *v = lcg(&mut seed) * 0.01;
        }

        let result = compute_rmt_filtered(&data, n, t);
        assert_eq!(result.filtered_corr.len(), n * n);

        // Diagonal should be 1.0.
        assert!((result.filtered_corr[0] - 1.0).abs() < 0.01);
        assert!((result.filtered_corr[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn correlated_tickers_have_factor() {
        // Two tickers that are identical should produce 1 factor.
        let t = 200;
        let n = 3;
        let mut data = vec![0.0_f32; n * t];

        let mut seed = 123u64;
        let lcg = |s: &mut u64| -> f32 {
            *s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((*s >> 33) as f32 / (1u64 << 31) as f32) - 1.0
        };

        // Ticker 0 and 1 are identical, ticker 2 is independent.
        for j in 0..t {
            let v = lcg(&mut seed) * 0.01;
            data[j] = v;
            data[t + j] = v;
            data[2 * t + j] = lcg(&mut seed) * 0.01;
        }

        let result = compute_rmt_filtered(&data, n, t);
        assert!(result.n_factors >= 1, "expected at least 1 factor, got {}", result.n_factors);
    }
}
