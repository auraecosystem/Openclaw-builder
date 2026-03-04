# Approximate Entropy (ApEn)
Regularity measure via template matching with tolerance. Lower ApEn = more predictable/regular patterns. Complements Permutation Entropy by using amplitude information rather than ordinal patterns.

## Use Cases
- Detecting consolidation (declining ApEn = increasingly regular price action)
- Regime classification: low ApEn = ordered coiling, high ApEn = chaotic
- Complement to PE: amplitude-sensitive where PE is scale-invariant
- EEG/physiological signal regularity (original domain)

## Limitations
- Self-matching bias inflates counts → underestimates complexity, especially short N
- Length-dependent: ApEn values vary with N, complicating cross-series comparison
- O(N²) — expensive for large windows
- SampEn is generally preferred (see sample-entropy.md)

## Algorithm

1. Form templates: `x_m(i) = [u(i), ..., u(i+m-1)]`
2. Chebyshev distance: `d[x_m(i), x_m(j)] = max_k |u(i+k) - u(j+k)|`
3. Count matches **including self**: `C_i^m(r) = |{j : d ≤ r}| / (N-m+1)`
4. `Φ^m(r) = (1/(N-m+1)) · Σ ln(C_i^m(r))`
5. **ApEn(m, r, N) = Φ^m(r) - Φ^{m+1}(r)**

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Window size | `period` | 200 | 100–1000 | Observations |
| Embedding dim | `m` | 2 | 1–3 | Template length |
| Tolerance | `r` | 0.2×std | 0.1–0.25×std | Match threshold |
| Tolerance mode | `r_mode` | relative | relative/absolute | How r scales |

## Complexity
O(N²) pairwise comparisons.

## Output
- `value: f64` — ApEn ∈ [0, ∞), typically [0, 2]

## References
Pincus (1991) PNAS 88:2297

## Status
To implement. Zero deps. Tier 3 (O(N²)).
