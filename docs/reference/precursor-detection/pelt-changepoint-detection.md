# PELT (Pruned Exact Linear Time) Change-Point Detection
Optimal offline segmentation that finds ALL changepoints in a series by minimizing penalized cost. Pruning makes it near-linear for series with many changepoints. Formalizes onset of contraction or excursion as distinct changepoint types.

## Use Cases
- Offline segmentation: find all structural breaks in a window
- Onset/termination boundary detection for events
- Complement to BOCPD: exact optimization vs online Bayesian posterior
- Multi-type: detect mean, variance, or joint mean+variance shifts

## Limitations
- Offline (batch) method — must re-run on sliding window for streaming
- O(n²) worst case when no changepoints (degrades to full DP)
- Penalty choice (BIC/AIC/manual) significantly affects results
- Type-specific cost function may miss "wrong" type of change

## Algorithm

**DP recurrence:**
```
F(t) = min_{τ∈R_t} [F(τ) + C(y_{(τ+1):t}) + β]
F(0) = -β
```

**Pruning:** remove τ* from R_t if `F(τ*) + C(y_{(τ*+1):s}) ≤ F(s)`.

**Cost functions:**
- Mean change: `C = Σ(y_i - ȳ)²`
- Variance change: `C = n·ln(σ̂²) + n`
- Mean+variance: `C = n·(ln(2π) + ln(σ̂²) + 1)`

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Penalty | `beta` | 2·ln(n) | AIC/BIC/manual | Per-changepoint cost |
| Cost function | `cost` | mean_var | mean/var/mean_var | Segment model |
| Min segment | `min_seg` | 2 | 2–50 | Min observations |
| Window | `period` | 500 | 100–2000 | Sliding window |

## Complexity
Best: O(n). Worst: O(n²). Typical: O(n log n).

## Output
- `value: f64` — changepoints / window_size (normalized)
- `changepoints: Vec<usize>` — positions
- `last_changepoint_age: usize` — bars since most recent

## References
Killick, Fearnhead & Eckley (2012) JASA 107(500)

## Status
To implement. Zero deps. Tier 3.
