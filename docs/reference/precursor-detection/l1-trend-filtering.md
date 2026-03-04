# L1 Trend Filtering

Piecewise-polynomial trend estimation via convex optimization. Finds the smoothest trend that fits
the data, with breakpoints (kinks) at structural transitions. More adaptive than Hodrick-Prescott;
kinks directly indicate regime boundaries.

## Use Cases

- Detrending with automatic breakpoint detection
- Event boundary estimation: kinks mark onset/termination
- Piecewise-linear trend extraction for monotone excursions
- Complement to Kalman: optimization-based vs Bayesian filtering

## Limitations

- Offline (batch) — must solve per window
- Lambda choice strongly affects smoothness vs fidelity
- ADMM convergence can be slow (hundreds of iterations)
- Heteroskedasticity biases trend-point locations unless robust loss used

## Algorithm

```
minimize (1/2)||y - θ||₂² + λ||D^{(k+1)}θ||₁
```

k=0: piecewise constant. k=1: piecewise linear. k=2: piecewise quadratic.

**ADMM:**

1. θ-update: solve `(I + ρ·D^T D)θ = y + ρ·D^T(z - u)` (banded Cholesky, O(n))
2. z-update: `z = S_{λ/ρ}(Dθ + u)`, soft-threshold `S_γ(v) = sign(v)·max(|v|-γ, 0)`
3. u-update: `u ← u + Dθ - z`

Stop when primal/dual residuals < tolerance.

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Window | `period` | 200 | 50–1000 | Sliding window |
| Regularization | `lambda` | 0.1×λ_max | 0.001–1.0×λ_max | Smoothness penalty |
| Polynomial order | `k` | 1 | 0/1/2 | Piecewise degree |
| ADMM penalty | `rho` | 1.0 | 0.01–100 | Lagrangian weight |
| Max iterations | `max_iter` | 500 | 100–5000 | ADMM limit |
| Abs tolerance | `eps_abs` | 1e-4 | 1e-8–1e-3 | Convergence |
| Rel tolerance | `eps_rel` | 1e-3 | 1e-5–1e-2 | Convergence |
| Adaptive rho | `adaptive_rho` | true | — | Auto-tune per Boyd 2011 |

λ_max = `||D^{(k+1)T}y||_∞`.

## Complexity

O(n) per ADMM iteration (banded Cholesky). Total: O(n × J), J≈100–1000.

## Output

- `value: f64` — filtered value at current bar
- `trend: Vec<f64>`, `knots: Vec<usize>`, `n_knots: usize`

## References

Kim et al. (2009) SIAM Review, Tibshirani (2014), Ramdas & Tibshirani (2016)

## Status

To implement. Zero deps. Tier 3.
