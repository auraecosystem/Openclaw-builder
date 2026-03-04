# Concept Drift Detection (DDM / Page-Hinkley / ADWIN)
Detects when the underlying data distribution changes, enabling rebaselining of precursor scores. Prevents false positives under long-term drift by distinguishing "regime changed" from "anomaly in current regime."

## Use Cases
- Wrapper around any precursor scorer: auto-rebaseline on drift
- Detecting structural regime changes vs transient anomalies
- Triggering parameter re-estimation for other indicators
- Online monitoring of stationarity assumptions

## Limitations
- Drift detectors can confuse rare true events with drift
- Adaptation risks forgetting recurring patterns (catastrophic forgetting)
- Coupling drift alarms to event alarms requires careful objective design
- DDM assumes Bernoulli error stream; not directly applicable to continuous values without discretization

## Algorithm

### DDM (Drift Detection Method)
```
p_t = errors_up_to_t / t
s_t = √(p_t · (1 - p_t) / t)
```
Warning: `p_t + s_t ≥ p_min + k_w · s_min`. Alarm: `p_t + s_t ≥ p_min + k_d · s_min`.

### Page-Hinkley
```
x̄_t = x̄_{t-1} + (x_t - x̄_{t-1}) / t
S_t = max(0, α · S_{t-1} + (x_t - x̄_t - δ))
```
Alarm when `S_t > λ`.

### ADWIN (Adaptive Windowing)
Variable-length window. For each split into sub-windows: alarm when `|μ̂₀ - μ̂₁| ≥ ε_cut`, where:
```
ε_cut = √(2 · m · var · d) + (2/3) · m · d
m = harmonic mean of sub-window sizes
d = ln(2 · ln(n) / δ)
```

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Method | `method` | adwin | ddm/page_hinkley/adwin | Detector type |
| DDM warning | `k_w` | 2.0 | 1.5–3.0 | Warning multiplier |
| DDM alarm | `k_d` | 3.0 | 2.0–4.0 | Alarm multiplier |
| PH tolerance | `delta` | 0.005 | 0.001–0.05 | Magnitude tolerance |
| PH threshold | `lambda` | 50.0 | 10–200 | Alarm threshold |
| PH forgetting | `alpha` | 0.9999 | 0.99–1.0 | Decay factor |
| ADWIN confidence | `adwin_delta` | 0.002 | 0.0005–0.01 | False alarm control |
| Min samples | `min_samples` | 30 | 10–100 | Burn-in |

## Complexity
DDM: O(1). Page-Hinkley: O(1). ADWIN: O(log n) amortized.

## Output
- `value: f64` — drift score
- `drift_detected: bool`
- `warning: bool` (DDM only)

## References
Gama et al. (2004), Page (1954), Bifet & Gavaldà (2007)

## Status
To implement. Zero deps. Tier 1 priority.
