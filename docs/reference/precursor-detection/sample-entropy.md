# Sample Entropy (SampEn)
Bias-corrected regularity measure. Like ApEn but excludes self-matches, removing the systematic bias that plagues short series. The preferred entropy measure for most applications.

## Use Cases
- Same as ApEn but with unbiased estimation
- Works with shorter series (N ≥ 200 vs N ≥ 1000 for ApEn)
- Cross-series comparison (less length-dependent)
- Consolidation detection in financial time series

## Limitations
- O(N²) — still expensive
- Can be undefined if A=0 (no (m+1)-length matches), returns +∞
- Still requires tolerance r parameter (sensitive to scaling)

## Algorithm

1. Templates: `x_m(i) = [u(i), ..., u(i+m-1)]` for i=1..N-m
2. Count m-length matches **excluding self** (j≠i): B = total count
3. Count (m+1)-length matches **excluding self**: A = total count
4. **SampEn(m, r, N) = -ln(A / B)**

### Key Advantages Over ApEn
- Unbiased (no self-match inflation)
- Monotone: SampEn(m+1) ≤ SampEn(m)
- Less length-dependent
- Works with N ≥ 200

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Window size | `period` | 200 | 100–1000 | Observations |
| Embedding dim | `m` | 2 | 1–3 | Template length |
| Tolerance | `r` | 0.2×std | 0.1–0.25×std | Match threshold |
| Tolerance mode | `r_mode` | relative | relative/absolute | How r scales |

## Complexity
O(N²).

## Output
- `value: f64` — SampEn ∈ [0, ∞), undefined→+∞ if A=0

## References
Richman & Moorman (2000) Am J Physiology 278:H2039

## Status
To implement. Zero deps. Tier 3 (O(N²)).
