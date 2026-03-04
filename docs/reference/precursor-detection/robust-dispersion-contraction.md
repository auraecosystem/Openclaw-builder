# Robust Rolling Dispersion / Contraction Index
Measures volatility contraction using robust dispersion estimators (MAD, IQR). Detects the "quiet before the storm" precursor pattern — dispersion dropping below historical baseline signals coiling.

## Use Cases
- Direct detection of volatility contraction preceding breakouts
- Robust alternative to ATR-based contraction (resists outliers)
- Cross-signal coupling: apply to both price and volume for dual-view
- Baseline for normalizing other indicator scores

## Limitations
- Window-length sensitivity (short vs long baseline trade-off)
- Heavy tails and intermittent bursts dominate unless robustified
- Nonstationary baselines create false contractions after regime changes
- Single-scale: misses multi-scale contraction patterns

## Algorithm

**MAD:** `MAD = median(|x_i - median(x)|)`, consistent σ̂ = 1.4826 × MAD

**IQR:** `IQR = Q₃ - Q₁`, consistent σ̂ = IQR / 1.3490

**Contraction ratio:**
```
contraction_t = D_short(t) / D_long(t)
```
D_short = dispersion over short window, D_long = dispersion over long baseline. Contraction when ratio < threshold.

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Short window | `period` | 20 | 5–50 | Current dispersion window |
| Long window | `baseline_period` | 100 | 50–252 | Baseline dispersion window |
| Contraction threshold | `threshold` | 0.5 | 0.2–0.8 | Alarm when ratio below |
| Dispersion metric | `metric` | `mad` | mad/iqr/std | Which measure |
| Scale factor | `scale` | 1.4826 | — | Gaussian consistency constant |

## Complexity
O(N log N) per window (median). O(N) with streaming median.

## Output
- `value: f64` — contraction ratio ∈ (0, ∞), <1 = contracting
- `dispersion_short: f64`, `dispersion_long: f64`
- `contracting: bool`

## References
Rousseeuw & Croux (1993), Leys et al. (2013)

## Status
To implement. Zero deps. Tier 1 priority.
