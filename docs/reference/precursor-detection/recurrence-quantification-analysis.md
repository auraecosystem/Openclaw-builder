# Recurrence Quantification Analysis (RQA)
Detects dynamical regime transitions via recurrence structure in phase-space reconstructions. Measures determinism, laminarity, and complexity of trajectories — captures nonlinear dynamics that linear methods miss.

## Use Cases
- Detecting transitions from chaotic to periodic dynamics (and vice versa)
- Regime classification: high DET = deterministic trending, high LAM = laminar/stuck
- Early warning of critical transitions (RQA measures change before amplitude)
- Complement to entropy measures: geometric rather than probabilistic view

## Limitations
- Requires embedding parameter choices (m, τ, ε) — results sensitive to these
- O(N²) time and space — expensive for large windows
- Heavy tails and measurement noise substantially alter recurrence structure
- Unstable for short windows (<50 embedded points)

## Algorithm

**Time-delay embedding:**
```
x_i = (s_i, s_{i+τ}, ..., s_{i+(m-1)τ})  for i=1..N'
N' = N - (m-1)τ
```

**Recurrence matrix:**
```
R_{i,j} = Θ(ε - ||x_i - x_j||)
```

**Measures** (P(l) = diagonal line histogram, P(v) = vertical line histogram):

- RR = (1/N'²) · Σ R_{i,j}
- DET = Σ_{l≥l_min} l·P(l) / Σ_{l≥1} l·P(l)
- L = Σ_{l≥l_min} l·P(l) / Σ_{l≥l_min} P(l)
- L_max; DIV = 1/L_max
- ENTR = -Σ p(l)·ln(p(l))
- LAM = Σ_{v≥v_min} v·P(v) / Σ_{v≥1} v·P(v)
- TT = Σ_{v≥v_min} v·P(v) / Σ_{v≥v_min} P(v)

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Window | `period` | 200 | 50–500 | Rolling window |
| Embedding dim | `m` | 3 | 2–10 | Phase-space dimension |
| Time delay | `tau` | 1 | 1–20 | Embedding delay |
| Threshold | `epsilon` | 0.1×std | 0.05–0.3×std | Recurrence distance |
| Epsilon mode | `epsilon_mode` | relative | relative/absolute/fixed_rr | Threshold type |
| Target RR | `target_rr` | 0.05 | 0.01–0.10 | For fixed_rr mode |
| Min diagonal | `l_min` | 2 | 2–5 | Minimum for DET |
| Min vertical | `v_min` | 2 | 2–5 | Minimum for LAM |
| Theiler window | `theiler` | 1 | 0–period | Diagonal exclusion |
| Norm | `norm` | euclidean | euclidean/chebyshev | Distance metric |

## Complexity
O(N'²) time and space.

## Output
- `value: f64` — DET (primary)
- `rr: f64`, `det: f64`, `lam: f64`, `entr: f64`, `l_mean: f64`, `tt: f64`

## References
Eckmann et al. (1987), Zbilut & Webber (1992), Marwan et al. (2007) Physics Reports 438

## Status
To implement. Zero deps. Tier 3 (O(N²)).
