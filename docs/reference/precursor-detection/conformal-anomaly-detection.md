# Conformal Anomaly Detection
Distribution-free anomaly scoring via calibrated p-values. Produces scores tied to false-alarm rates without distributional assumptions. The exchangeability martingale detects when the i.i.d. assumption breaks (drift/regime change).

## Use Cases
- Calibrated anomaly scores with guaranteed false-alarm rates
- Distribution-free uncertainty quantification for any indicator
- Detecting exchangeability violations (structural breaks via martingale)
- Post-hoc calibration wrapper for raw anomaly scores from other methods

## Limitations
- Conformal validity requires exchangeability — strained by changepoints
- kNN nonconformity is O(cal_size × dim) per observation
- Calibration window must be representative of "normal"
- Martingale can alarm slowly on gradual drift

## Algorithm

**Nonconformity:** `α_t = d_k(x_t)` (k-NN distance to calibration set).

**p-value (inductive):**
```
p_t = (|{i ∈ D_cal : α_i ≥ α_t}| + 1) / (|D_cal| + 1)
```

**Power martingale:**
```
M_n = Π_{i=1}^{n} ε · p_i^{ε-1}
```
Alarm when `M_n ≥ 1/δ`.

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Calibration window | `period` | 200 | 50–1000 | Cal set size |
| k (neighbors) | `k` | 5 | 1–20 | kNN parameter |
| Nonconformity fn | `ncm` | knn | knn/avg_knn/residual | Measure type |
| Significance | `epsilon` | 0.05 | 0.01–0.10 | FPR target |
| Martingale power | `mart_power` | 0.5 | 0.2–0.9 | Power param ε |
| Martingale threshold | `mart_threshold` | 20.0 | 10–1000 | 1/δ alarm |
| Use martingale | `use_martingale` | true | — | Exchangeability test |

## Complexity
Per observation: O(|D_cal| × dim). Martingale: O(1).

## Output
- `value: f64` — p-value ∈ (0, 1]
- `martingale: f64`, `anomaly: bool`

## References
Vovk et al. (2005), Laxhammar & Falkman (2014)

## Status
Already implemented in engine (`conformal.rs`). To port to NT. Tier 2.
