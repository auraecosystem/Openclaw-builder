# Scan Statistics
Sliding-window likelihood ratio test that detects localized anomalies by comparing local statistics to baseline. Originally from epidemiology (outbreak detection); naturally provides ensemble-relative scores by ranking streams against population baselines.

## Use Cases
- Ensemble-relative scoring: rank series by deviation from population norm
- Localized anomaly detection: find the most anomalous time window
- Outbreak-style surveillance across many parallel series
- Complementary to changepoint detection: finds "where" the anomaly is

## Limitations
- Baseline stationarity assumption violated by drift
- Multiple-testing across many streams needs correction
- Population-wide drift can mask or amplify individual series signals
- Monte Carlo p-values are computationally expensive

## Algorithm

**Gaussian LLR for window [s, s+w-1]:**
```
LLR(s,w) = (n/2)·ln(σ₀²) - (n/2)·ln(σ_z²)
```
σ₀² = MLE under H₀ (common mean), σ_z² = MLE under H₁ (different mean inside/outside).

**Scan statistic:** `S = max_{s,w} LLR(s,w)` over all positions and window sizes.

**Poisson variant:**
```
LLR(z) = n_z·ln(n_z/μ_z) + (N-n_z)·ln((N-n_z)/(N-μ_z))
```

**p-value:** Monte Carlo — simulate M datasets under H₀, compute S for each, p = (R+1)/(M+1).

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Window | `period` | 500 | 100–2000 | Observation window |
| Min scan window | `w_min` | 5 | 2–20 | Smallest scan |
| Max scan window | `w_max` | period/2 | — | Largest scan |
| Baseline window | `baseline_period` | 200 | 50–500 | Reference |
| Distribution | `distribution` | gaussian | gaussian/poisson | Model |
| Significance | `alpha` | 0.05 | 0.01–0.10 | p-value cutoff |
| MC reps | `n_mc` | 999 | 99–9999 | For p-value |

## Complexity
O(n × W) per scan. O(M × n × W) with Monte Carlo.

## Output
- `value: f64` — max LLR (normalized)
- `best_window_start: usize`, `best_window_size: usize`
- `p_value: f64`

## References
Kulldorff (1997), Glaz et al. (2001)

## Status
To implement. Zero deps. Tier 2.
