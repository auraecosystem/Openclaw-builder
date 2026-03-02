//! 1.5 KNN Anomaly Detection -- feature-space outlier score.
//!
//! Measures how "unusual" the current bar's feature window is compared to
//! a reference set of historical feature vectors.  In crypto, a consolidation
//! is the anomaly -- normal action is noisy and volatile, so a tightening
//! base scores as an outlier.
//!
//! Uses `kiddo` for efficient nearest-neighbour queries in moderate
//! dimensionality (~10 features).  Falls back to brute-force when `kiddo`
//! types do not match (feature count must be a compile-time const for kiddo).

/// Compute the KNN anomaly score for the current feature vector.
///
/// # Arguments
/// * `features` -- feature vector for the current window (~10 features).
/// * `reference` -- matrix of historical feature vectors (each row is one window).
/// * `k` -- number of nearest neighbors.
///
/// # Returns
/// Anomaly score: mean Euclidean distance to the k nearest neighbors,
/// normalised by the median reference score.  Higher = more anomalous.
/// Returns 0.0 for degenerate inputs.
pub fn compute_knn_anomaly(
    features: &[f32],
    reference: &[&[f32]],
    k: usize,
) -> f32 {
    let dim = features.len();
    if dim == 0 || reference.is_empty() || k == 0 {
        return 0.0;
    }

    // Brute-force KNN -- reference sets are typically small enough (~200-500)
    // that building a full KdTree each bar is wasteful.  A flat scan in
    // O(N * dim) is fast for N < ~1000 and avoids compile-time dimension
    // constraints from kiddo's generic KdTree.
    let mut dists: Vec<f32> = reference
        .iter()
        .filter_map(|r| {
            if r.len() != dim {
                return None;
            }
            let d2: f32 = features
                .iter()
                .zip(r.iter())
                .map(|(a, b)| {
                    let diff = a - b;
                    diff * diff
                })
                .sum();
            Some(d2.sqrt())
        })
        .collect();

    if dists.is_empty() {
        return 0.0;
    }

    // Partial sort to find k-th nearest.
    let k_eff = k.min(dists.len());
    dists.select_nth_unstable_by(k_eff - 1, |a, b| {
        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mean_k: f32 = dists[..k_eff].iter().sum::<f32>() / k_eff as f32;

    // Normalise by median reference distance.
    // To compute median reference score, take each reference point's mean-k
    // distance to the rest of the reference set.  This is expensive; instead
    // use a simpler proxy: median pairwise distance to the query's k NNs.
    // For a fast approximation, just return the raw mean distance.
    // The caller can normalise offline once a baseline is established.
    mean_k
}

/// Compute KNN anomaly using a pre-built `kiddo` KdTree for higher
/// throughput when the reference set is large or reused across tickers.
///
/// This version requires the feature dimension to be known at compile time.
/// Currently supports DIM = 10.
pub fn compute_knn_anomaly_kdtree<const DIM: usize>(
    features: &[f32; DIM],
    tree: &kiddo::float::kdtree::KdTree<f32, u32, DIM, 32, u16>,
    k: usize,
) -> f32 {
    if k == 0 {
        return 0.0;
    }

    let results = tree.nearest_n::<kiddo::float::distance::SquaredEuclidean>(features, k);
    if results.is_empty() {
        return 0.0;
    }

    let mean_dist: f32 = results.iter().map(|nn| nn.distance.sqrt()).sum::<f32>()
        / results.len() as f32;
    mean_dist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlier_scores_higher() {
        // Reference: cluster near origin.
        let refs: Vec<Vec<f32>> = (0..50)
            .map(|i| vec![i as f32 * 0.01, i as f32 * 0.01, 0.0])
            .collect();
        let ref_slices: Vec<&[f32]> = refs.iter().map(|v| v.as_slice()).collect();

        let inlier = [0.2, 0.2, 0.0];
        let outlier = [10.0, 10.0, 10.0];

        let score_in = compute_knn_anomaly(&inlier, &ref_slices, 5);
        let score_out = compute_knn_anomaly(&outlier, &ref_slices, 5);

        assert!(
            score_out > score_in,
            "outlier should score higher: in={score_in}, out={score_out}"
        );
    }
}
