# EWMA (Exponentially Weighted Moving Average) Control Chart
Smoothed tracking of process mean with time-varying control limits. More sensitive to small shifts than Shewhart charts; naturally weights recent observations more.

## Use Cases
- Detecting gradual drift in return distributions
- Volatility regime monitoring (EWMA variance tracking)
- Adaptive baseline estimation for other indicators
- Online quality control with configurable sensitivity

## Limitations
- Requires known or well-estimated mu_0 and sigma
- Lambda choice is shift-size-dependent (no universal default)
- At lambda=1, degrades to Shewhart chart (no memory)
- Heavy tails inflate false alarms without robust sigma estimation

## Algorithm

```
Z_t = λ · x_t + (1 - λ) · Z_{t-1},  Z_0 = μ₀
```

**Time-varying control limits:**
```
UCL_t = μ₀ + L · σ · √(λ/(2-λ) · (1-(1-λ)^(2t)))
LCL_t = μ₀ - L · σ · √(λ/(2-λ) · (1-(1-λ)^(2t)))
```

**Steady-state limits** (t → ∞):
```
UCL = μ₀ + L · σ · √(λ/(2-λ))
```

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Smoothing constant | `lambda` | 0.2 | 0.05–0.3 | Current observation weight |
| Control limit width | `L` | 3.0 | 2.5–3.5 | Sigma multiplier |
| Target mean | `mu_0` | estimated | — | In-control mean |
| Target std | `sigma` | estimated | — | In-control std dev |
| Estimation window | `period` | 200 | 50–1000 | For parameter estimation |
| Steady-state limits | `steady_state` | false | — | Skip transient term |

**Recommended combos** (ARL₀ ≈ 500): (λ=0.05, L=2.615) small shifts; (λ=0.20, L=2.962) moderate.

## Complexity
O(1) per observation.

## Output
- `value: f64` — Z_t EWMA statistic
- `ucl: f64` — upper control limit
- `lcl: f64` — lower control limit
- `alarm: bool` — out of limits

## References
Roberts (1959), Lucas & Saccucci (1990)

## Status
To implement. Zero external deps. Tier 1 priority.
