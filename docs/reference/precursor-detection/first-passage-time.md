# First-Passage Time / Hitting-Time Model

Probabilistic model for threshold-crossing events. Given estimated drift and volatility, computes
the probability of price reaching a barrier within a time horizon. Uses the Inverse Gaussian
distribution (exact for Brownian motion with drift).

## Use Cases

- Excursion termination modeling: P(price crosses target within T bars)
- Position sizing via expected time to stop-loss/target
- Event boundary estimation: when will the current move end?
- Complement to survival models: parametric vs nonparametric

## Limitations

- Assumes Brownian motion (Gaussian increments, constant drift/vol)
- Nonstationarity and structural breaks violate assumptions
- Empirical drift/volatility estimates are noisy for short windows
- Non-monotone hazard (inverse Gaussian) may not match reality

## Algorithm

**CDF for first-passage to level a (Brownian motion with drift μ, vol σ):**

```
P(T_a ≤ t) = Φ((μt - a)/(σ√t)) + exp(2μa/σ²)·Φ(-(μt + a)/(σ√t))
```

**Moments:** E[T] = a/μ, Var[T] = a·σ²/μ³.

**Hazard:** h(t) = f(t) / (1 - F(t)), non-monotone (rises then falls).

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Estimation window | `period` | 100 | 30–500 | For μ, σ |
| Threshold up | `threshold_up` | 2.0×σ | 1–5×σ | Upper barrier |
| Threshold down | `threshold_down` | 2.0×σ | 1–5×σ | Lower barrier |
| Symmetric | `symmetric` | true | — | Same both ways |
| Drift method | `drift_method` | ols | ols/median_slope | Estimator |
| Vol method | `vol_method` | mad | mad/std | Estimator |

## Complexity

O(1) per observation; O(N) for estimation.

## Output

- `value: f64` — P(T ≤ horizon), survival probability
- `hazard: f64`, `expected_time: f64`, `elapsed: usize`

## References

Wald (1947), Inverse Gaussian distribution

## Status

To implement. Zero deps. Tier 2.
