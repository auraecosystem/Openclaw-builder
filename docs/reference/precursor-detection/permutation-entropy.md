# Permutation Entropy (PE)
Ordinal complexity measure via Shannon entropy of permutation patterns. PE ∈ [0,1]: 0 = perfectly ordered (coiling), 1 = random. Detects consolidation as declining PE — price action becomes more predictable before breakouts.

## Use Cases
- Consolidation detection: falling PE signals increasingly structured (coiling) price action
- Regime complexity classification: high PE = choppy, low PE = trending or coiling
- Tipping-point early warning: PE minimum often precedes regime transitions

## Limitations
- Sensitive to embedding order choice (`order`): higher order captures longer-range structure but needs more data
- Nonstationary trends masquerade as entropy shifts — use on returns rather than raw prices when in doubt
- Sampling-rate dependent: PE at daily bars differs from 5m bars for the same instrument

## Algorithm

1. Form delay-embedded patterns of length `order` with stride `delay` from the price window
2. Encode each pattern as a Lehmer (factorial number system) index for O(order²) collision-free hashing
3. Compute Shannon entropy over the pattern frequency distribution
4. Normalize by ln(order!) so output is in [0, 1]

Normalization: `PE = -Σ p(π) ln p(π) / ln(order!)`

## Parameters

| Parameter | Default | Constraint | Description |
|---|---|---|---|
| `period` | — | ≥ 1 | Rolling window length (price samples) |
| `order` | 3 | ≥ 2 | Embedding dimension (pattern length) |
| `delay` | 1 | ≥ 1 | Stride between embedding samples |

Minimum samples before first valid output: `(order - 1) * delay + 1`. Initialized after `period` inputs.

## Output
- `value: f64` — PE ∈ [0, 1] (0 = ordered, 1 = maximum entropy)

## Status
Implemented. Engine: `perm_entropy.rs`. NT: `PermutationEntropy`.
