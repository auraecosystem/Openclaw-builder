# Quickest Change Detection in AR Processes

CUSUM applied to autoregressive residuals. Fits AR(p) on a reference window, computes residuals on
live data, and runs CUSUM on those residuals. Detects when the data-generating process changes from
the reference regime — more powerful than raw CUSUM because it removes serial correlation.

## Use Cases

- Detecting regime changes in autocorrelated series (most financial data)
- More powerful than raw CUSUM: accounts for serial dependence
- AR residual monitoring: structural break in the generating process
- Complement to BOCPD: frequentist, faster, simpler

## Limitations

- AR order p must be chosen (AIC/BIC); misspecification reduces power
- Reference window must be long enough for stable AR estimation
- Stale AR coefficients need periodic re-fitting
- Assumes Gaussian residuals (heavy tails inflate false alarms)

## Algorithm

1. Fit AR(p) on reference window: `x_t = Σ φ_j·x_{t-j} + ε_t`
2. Residuals: `e_t = x_t - Σ φ_j·x_{t-j}`
3. Two-sided CUSUM on residuals:

```
g_t+ = max(0, g_{t-1}+ + e_t - k)
g_t- = max(0, g_{t-1}- - e_t - k)
```

4. Alarm when `max(g+, g-) > h`

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| AR order | `p` | 4 | 1–10 | Autoregressive order |
| Reference window | `ref_period` | 200 | 50–1000 | AR training window |
| Test window | `period` | 100 | 20–500 | Sliding test window |
| CUSUM threshold | `h` | 5.0 | 3.0–8.0 | Alarm threshold |
| Reference value | `k` | 0.5 | 0.25–1.0 | Allowance |
| AR method | `ar_method` | yule_walker | yule_walker/ols | Fitting method |
| Two-sided | `two_sided` | true | — | Both directions |
| Re-fit interval | `refit_interval` | 100 | 50–500 | Re-estimate AR |

## Complexity

AR fitting: O(N_ref · p²). Per observation: O(p).

## Output

- `value: f64` — max CUSUM on residuals
- `alarm: bool`, `residual: f64`

## References

Lai (1998) IEEE Trans IT, Tartakovsky et al. (2014)

## Status

To implement. Zero deps. Tier 2.
