# KNN Anomaly Detection
Distance-based anomaly scoring via k-nearest neighbors in feature space. Measures how unusual the current bar's feature vector is relative to a reference set of historical windows. In crypto momentum, consolidation is the anomaly — normal price action is noisy and volatile, so a tightening base scores as an outlier.

## Use Cases
- Detecting compression/consolidation as an outlier in a volatile reference set
- Multi-feature anomaly scoring without distributional assumptions
- Pre-breakout pattern detection by identifying atypical low-volatility windows
- Complement to conformal anomaly for non-calibrated distance scores

## Limitations
- Feature engineering entirely determines signal quality — garbage in, garbage out
- Brute-force O(N × dim) per bar; efficient for reference sets under ~1000 windows
- No calibration: raw mean distance requires offline baseline normalization
- KdTree variant requires compile-time feature dimension (currently fixed at DIM=10)
- Sensitive to feature scaling — features must be normalized before comparison

## Algorithm

**Brute-force variant:**
```
For current feature vector x and reference set R:
  dists = [euclidean(x, r) for r in R if len(r) == dim]
  Partial-sort dists; take k_eff = min(k, |dists|) nearest
  score = mean(dists[:k_eff])
```

**KdTree variant (compile-time DIM):**
```
results = tree.nearest_n::<SquaredEuclidean>(features, k)
score = mean([sqrt(nn.distance) for nn in results])
```

Both variants return raw mean Euclidean distance. The brute-force path uses `select_nth_unstable_by` (partial sort) rather than full sort — O(N) average case for the k-selection step.

## Parameters

| Parameter | Default | Range | Description |
|---|---|---|---|
| `k` | 5 | 1–50 | Number of nearest neighbors to average |
| `dim` (implicit) | 10 | 3–20 | Feature vector dimensionality |
| Reference size | 200–500 | 50–2000 | Number of historical windows in reference set |

## Complexity
Brute-force: O(N × dim) per bar for distance computation, O(N) for partial sort.
KdTree: O(dim × log N) per bar after O(N × dim × log N) tree build.

Brute-force preferred for N < ~1000 because tree rebuild cost per bar exceeds the scan.

## Output
- `score: f32` — mean Euclidean distance to k nearest neighbors (higher = more anomalous)
- Unnormalized; caller normalizes by baseline median score offline

## References
Ramaswamy et al. (2000) — distance-based outlier detection
Breunig et al. (2000) — LOF (related: local outlier factor)
Kiddo crate (Rust) — compile-time KdTree

## Status
Implemented in engine only (`knn_anomaly.rs`). Not ported to NautilusTrader. Tier 2.
