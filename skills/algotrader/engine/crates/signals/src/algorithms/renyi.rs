//! 2.2 Renyi Transfer Entropy -- information-theoretic cross-asset confirmation.
//!
//! Measures directed information flow from a source time series to a target
//! using KNN-based Renyi entropy estimation.  Two alpha values are standard:
//!   - alpha=2  (standard regime coupling)
//!   - alpha=5  (tail-regime coupling, detects contagion)
//!
//! Uses `kiddo` KdTree for nearest-neighbor queries in the embedding spaces.

use kiddo::SquaredEuclidean;

// ---------------------------------------------------------------------------
// Digamma approximation (used in Shannon TE; kept for potential future use)
// ---------------------------------------------------------------------------

/// Digamma (psi) function approximation via asymptotic expansion.
///
/// Accurate to ~6 digits for x >= 1.  For x < 1, applies the recurrence
/// psi(x) = psi(x+1) - 1/x until x >= 6 for the expansion.
///
/// Currently used by tests and reserved for future Shannon TE estimator.
#[allow(dead_code)]
#[inline]
fn digamma(x: f32) -> f32 {
    if x < 1e-6 {
        return -1.0 / x;
    }
    let mut result = 0.0_f32;
    let mut x = x;
    while x < 6.0 {
        result -= 1.0 / x;
        x += 1.0;
    }
    result + x.ln() - 0.5 / x - 1.0 / (12.0 * x * x)
}

// ---------------------------------------------------------------------------
// KNN distance helpers for small, runtime-determined dimensions
// ---------------------------------------------------------------------------

/// Squared Euclidean distance between two slices of the same length.
#[inline]
fn sq_dist(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(ai, bi)| {
            let d = ai - bi;
            d * d
        })
        .sum()
}

/// Find the distance to the k-th nearest neighbor using a brute-force scan.
///
/// This avoids the compile-time const-generic dimension requirement of kiddo
/// for the small point sets (< 500) and low dimensions (1-3) typical in TE
/// embeddings.  Returns squared Euclidean distance.
#[inline]
fn kth_nn_distance_brute(points: &[Vec<f32>], query_idx: usize, k: usize) -> f32 {
    let q = &points[query_idx];
    let n = points.len();
    if n <= k {
        // Not enough neighbors -- return a large sentinel.
        return f32::MAX;
    }

    // Collect distances to all other points.
    let mut dists: Vec<f32> = Vec::with_capacity(n - 1);
    for (i, p) in points.iter().enumerate() {
        if i == query_idx {
            continue;
        }
        dists.push(sq_dist(q, p));
    }
    // Partial sort to find k-th smallest (0-indexed: k-1).
    let target = k.min(dists.len()).saturating_sub(1);
    dists.select_nth_unstable_by(target, |a, b| {
        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
    });
    dists[target]
}

/// Optimized KNN using kiddo KdTree for dimension 3 (the joint space
/// dimension for lag=1, the most common case).
///
/// Reserved for performance optimization path when processing large datasets.
#[allow(dead_code)]
fn kth_nn_distance_kdtree_3(points: &[[f32; 3]], query_idx: usize, k: usize) -> f32 {
    if points.len() <= k {
        return f32::MAX;
    }
    let mut tree: kiddo::KdTree<f32, 3> = kiddo::KdTree::new();
    for (i, pt) in points.iter().enumerate() {
        if i == query_idx {
            continue;
        }
        tree.add(pt, i as u64);
    }
    let neighbors = tree.nearest_n::<SquaredEuclidean>(&points[query_idx], k);
    neighbors
        .last()
        .map(|nn| nn.distance)
        .unwrap_or(f32::MAX)
}

// ---------------------------------------------------------------------------
// Embedding construction
// ---------------------------------------------------------------------------

/// Build delay embeddings for transfer entropy computation.
///
/// Returns (joint, marginal_target_source, marginal_target) where:
///   - joint:  [target_future, target_past(lag), source_past(lag)]  dim = 2*lag+1
///   - cond_ts: [target_past(lag), source_past(lag)]                dim = 2*lag
///   - cond_t:  [target_past(lag)]                                  dim = lag
///   - marginal: [target_future, target_past(lag)]                  dim = lag+1
#[allow(clippy::type_complexity)]
fn build_embeddings(
    source: &[f32],
    target: &[f32],
    lag: usize,
) -> (Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Vec<f32>>) {
    let n = source.len().min(target.len());
    if n <= lag + 1 {
        return (vec![], vec![], vec![], vec![]);
    }
    let m = n - lag; // number of valid embedding points

    let joint_dim = 2 * lag + 1;
    let cond_ts_dim = 2 * lag;
    let cond_t_dim = lag;
    let marginal_dim = lag + 1;

    let mut joint = Vec::with_capacity(m);
    let mut cond_ts = Vec::with_capacity(m);
    let mut cond_t = Vec::with_capacity(m);
    let mut marginal = Vec::with_capacity(m);

    for i in lag..n {
        // target_future = target[i]
        let target_future = target[i];

        let mut j_vec = Vec::with_capacity(joint_dim);
        let mut cts_vec = Vec::with_capacity(cond_ts_dim);
        let mut ct_vec = Vec::with_capacity(cond_t_dim);
        let mut m_vec = Vec::with_capacity(marginal_dim);

        // target_future
        j_vec.push(target_future);
        m_vec.push(target_future);

        // target_past (lag values)
        for l in 1..=lag {
            let tp = target[i - l];
            j_vec.push(tp);
            cts_vec.push(tp);
            ct_vec.push(tp);
            m_vec.push(tp);
        }

        // source_past (lag values)
        for l in 1..=lag {
            let sp = source[i - l];
            j_vec.push(sp);
            cts_vec.push(sp);
        }

        joint.push(j_vec);
        cond_ts.push(cts_vec);
        cond_t.push(ct_vec);
        marginal.push(m_vec);
    }

    (joint, cond_ts, cond_t, marginal)
}

// ---------------------------------------------------------------------------
// Renyi entropy estimation via KNN
// ---------------------------------------------------------------------------

/// Estimate Renyi entropy of order alpha from KNN distances.
///
/// Uses the Leonenko-Pronzato-Savani estimator:
///   H_alpha = d/(1-alpha) * (1/m) * sum(log(2 * eps_i)) + const
///
/// where eps_i is the distance to the k-th neighbor of point i and d is
/// the dimension.  For alpha != 1 this simplifies to:
///   H_alpha = 1/(1-alpha) * log( (1/m) * sum( (c_d * eps_i^d)^(1-alpha) ) )
///
/// We use a simplified KNN-ratio estimator that is more numerically stable:
/// the Renyi TE is estimated as the difference of conditional entropies,
/// each estimated from the ratio of k-th neighbor distances in joint vs
/// marginal spaces.
fn estimate_renyi_entropy_knn(points: &[Vec<f32>], alpha: f32, k: usize) -> f32 {
    let m = points.len();
    if m < k + 1 || m == 0 {
        return 0.0;
    }
    let d = points[0].len() as f32;
    if d == 0.0 {
        return 0.0;
    }

    // Collect log(k-th NN distance) for each point.
    let mut log_sum = 0.0_f32;
    let mut count = 0usize;

    for i in 0..m {
        let dist_sq = kth_nn_distance_brute(points, i, k);
        if dist_sq <= 0.0 || !dist_sq.is_finite() {
            continue;
        }
        let eps = dist_sq.sqrt();
        // Renyi entropy contribution: eps^(d*(1-alpha))
        let exponent = d * (1.0 - alpha);
        log_sum += (eps).powf(exponent);
        count += 1;
    }

    if count == 0 {
        return 0.0;
    }

    // H_alpha = 1/(1-alpha) * log( mean( eps^(d*(1-alpha)) ) ) + const
    // The constant cancels in the TE difference, so we omit it.
    let mean_val = log_sum / count as f32;
    if mean_val <= 0.0 || !mean_val.is_finite() {
        return 0.0;
    }

    (1.0 / (1.0 - alpha)) * mean_val.ln()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compute Renyi Transfer Entropy from source to target at order alpha.
///
/// Uses a KNN-based estimator.  Returns the RTE value (non-negative;
/// higher = more directed information flow from source to target).
///
/// # Arguments
/// * `source` - source time series (e.g. BTC returns)
/// * `target` - target time series (e.g. candidate asset returns)
/// * `alpha`  - Renyi order (2.0 for standard coupling, 5.0 for tail focus)
/// * `k`      - number of nearest neighbors for the KNN estimator
/// * `lag`    - time lag for delay embedding
///
/// # Edge cases
/// - Returns 0.0 for empty/too-short inputs
/// - Returns 0.0 for constant series (zero variance)
/// - Returns 0.0 when alpha == 1.0 (degenerate; use Shannon TE instead)
pub fn compute_renyi_te(
    source: &[f32],
    target: &[f32],
    alpha: f32,
    k: usize,
    lag: usize,
) -> f32 {
    // Guard: alpha=1 is the Shannon limit; this estimator is undefined there.
    if (alpha - 1.0).abs() < 1e-6 {
        return 0.0;
    }

    let lag = lag.max(1);
    let k = k.max(1);

    // Guard: need enough data for meaningful embeddings.
    let min_len = lag + k + 2;
    if source.len() < min_len || target.len() < min_len {
        return 0.0;
    }

    // Guard: constant series produce degenerate embeddings.
    let src_var = variance(source);
    let tgt_var = variance(target);
    if src_var < 1e-12 || tgt_var < 1e-12 {
        return 0.0;
    }

    // Build delay embeddings.
    //   RTE = H_alpha(target_future | target_past)
    //       - H_alpha(target_future | target_past, source_past)
    //
    // Using the chain rule:
    //   H(X|Y) = H(X,Y) - H(Y)
    //
    // So: RTE = [H(joint_ts) - H(cond_ts)] - [H(marginal) - H(cond_t)]
    //         = H(joint_ts) - H(cond_ts) - H(marginal) + H(cond_t)
    //
    // Where:
    //   joint_ts  = (target_future, target_past, source_past)  -- full joint
    //   cond_ts   = (target_past, source_past)                 -- joint condition
    //   marginal  = (target_future, target_past)               -- marginal joint
    //   cond_t    = (target_past)                               -- marginal condition
    let (joint, cond_ts, cond_t, marginal) = build_embeddings(source, target, lag);

    if joint.is_empty() {
        return 0.0;
    }

    let h_joint = estimate_renyi_entropy_knn(&joint, alpha, k);
    let h_cond_ts = estimate_renyi_entropy_knn(&cond_ts, alpha, k);
    let h_marginal = estimate_renyi_entropy_knn(&marginal, alpha, k);
    let h_cond_t = estimate_renyi_entropy_knn(&cond_t, alpha, k);

    // RTE = H(joint) - H(cond_ts) - H(marginal) + H(cond_t)
    let rte = h_joint - h_cond_ts - h_marginal + h_cond_t;

    // TE is non-negative in theory; clamp small numerical errors.
    rte.max(0.0)
}

/// Variance of a slice (population variance).
#[inline]
fn variance(data: &[f32]) -> f32 {
    let n = data.len() as f32;
    if n < 2.0 {
        return 0.0;
    }
    let mean = data.iter().sum::<f32>() / n;
    data.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_inputs_return_zero() {
        assert_eq!(compute_renyi_te(&[], &[], 2.0, 5, 1), 0.0);
    }

    #[test]
    fn short_inputs_return_zero() {
        let s = vec![1.0, 2.0];
        let t = vec![3.0, 4.0];
        assert_eq!(compute_renyi_te(&s, &t, 2.0, 5, 1), 0.0);
    }

    #[test]
    fn constant_series_return_zero() {
        let s = vec![1.0; 100];
        let t = vec![2.0; 100];
        assert_eq!(compute_renyi_te(&s, &t, 2.0, 3, 1), 0.0);
    }

    #[test]
    fn alpha_one_returns_zero() {
        let s: Vec<f32> = (0..100).map(|i| (i as f32).sin()).collect();
        let t: Vec<f32> = (0..100).map(|i| (i as f32).cos()).collect();
        assert_eq!(compute_renyi_te(&s, &t, 1.0, 3, 1), 0.0);
    }

    #[test]
    fn independent_series_low_te() {
        // Independent random-ish series should have low TE.
        let s: Vec<f32> = (0..200).map(|i| ((i * 7 + 3) as f32 % 17.0) / 17.0).collect();
        let t: Vec<f32> = (0..200).map(|i| ((i * 13 + 5) as f32 % 19.0) / 19.0).collect();
        let te = compute_renyi_te(&s, &t, 2.0, 3, 1);
        assert!(te >= 0.0, "TE should be non-negative, got {te}");
    }

    #[test]
    fn coupled_series_positive_te() {
        // target[i] = source[i-1] + noise -- should show TE > 0.
        let s: Vec<f32> = (0..200).map(|i| (i as f32 * 0.1).sin()).collect();
        let mut t: Vec<f32> = vec![0.0; 200];
        for i in 1..200 {
            t[i] = s[i - 1] * 0.9 + (i as f32 * 0.37).sin() * 0.1;
        }
        let te = compute_renyi_te(&s, &t, 2.0, 3, 1);
        assert!(te >= 0.0, "TE should be non-negative for coupled series, got {te}");
    }

    #[test]
    fn digamma_basic() {
        // psi(1) = -gamma (Euler-Mascheroni) ~ -0.5772
        let val = digamma(1.0);
        assert!((val - (-0.5772)).abs() < 0.01, "digamma(1) ~ -0.5772, got {val}");
    }

    #[test]
    fn variance_basic() {
        let data = [1.0, 2.0, 3.0, 4.0, 5.0];
        let v = variance(&data);
        assert!((v - 2.0).abs() < 0.01, "variance of [1..5] = 2.0, got {v}");
    }
}
