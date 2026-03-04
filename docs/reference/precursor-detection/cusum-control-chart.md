# CUSUM (Cumulative Sum Control Chart)
Sequential change detection via cumulative deviation tracking. Detects small persistent shifts in mean that single-observation tests miss.

## Use Cases
- Online detection of mean shifts in price returns or volatility
- Onset/termination boundary detection for excursion events
- Structural break alarm in stationary-assumption indicators
- Complement to BOCPD: faster, simpler, no Bayesian overhead

## Limitations
- Assumes known pre/post-change distributions (Gaussian standard)
- Sensitive to mu_0/sigma misspecification under heavy tails
- Type-specific: mean-shift CUSUM misses pure variance changes
- Dependence across series complicates null calibration

## Algorithm

**One-sided upper** (detects upward shifts):
```
S+(t) = max(0, S+(t-1) + (x_t - μ₀)/σ - k)
```

**One-sided lower** (detects downward shifts):
```
S-(t) = max(0, S-(t-1) - (x_t - μ₀)/σ + k)
```

Initial: S+(0) = S-(0) = 0. Alarm when S+(t) > h or S-(t) > h.

**FIR variant:** S+(0) = S-(0) = h/2 (head-start for early sensitivity).

**Design rule:** For shift of δ sigma units: k = δ/2. ARL₀ ≈ exp(h) / D(f₁ ∥ f₀).

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Reference value | `k` | 0.5 | 0.25–1.0 | Half the target shift (sigma units) |
| Decision interval | `h` | 5.0 | 3.0–8.0 | Alarm threshold |
| Target mean | `mu_0` | estimated | — | In-control mean |
| Target std | `sigma` | estimated | — | In-control std dev |
| Estimation window | `period` | 200 | 50–1000 | Window for mu_0/sigma estimation |
| FIR enabled | `fir` | false | — | Fast initial response |
| Two-sided | `two_sided` | true | — | Detect both directions |

## Complexity
O(1) per observation. O(n) total.

## Output
- `value: f64` — max(S+, S-), the CUSUM statistic
- `alarm: bool` — threshold exceeded
- `upper: f64` — S+ value
- `lower: f64` — S- value

## References
Page (1954), Lucas & Crosier (1982), Hawkins (1987)

## Status
To implement. Zero external deps. Tier 1 priority.
