# Matrix Profile (STOMP/MASS)
STOMP algorithm — minimum z-normalized Euclidean distance between current subsequence and all historical subsequences. Low = motif match (familiar pattern), high = novelty (unseen behavior). Uses FFT-accelerated MASS for O(N log N) sliding dot products.

## Use Cases
- Pattern novelty detection: high score flags price action that has no historical precedent in the window
- Motif discovery: low score identifies recurring patterns (useful for entry timing)
- Regime change warning: a sudden jump in novelty score often precedes structural breaks

## Limitations
- Constant subsequences (zero std) are skipped — cannot measure novelty in a flat market
- Novelty mapping `1/(1+dist)` compresses the upper range; large distances are all mapped near 1.0
- `subsequence_len` must be less than `period` and chosen relative to the pattern timescale of interest
- Computationally heavy: FFT convolution on every bar update

## Algorithm

MASS (Mueen's Algorithm for Similarity Search):

1. Compute z-normalized query from the most-recent `subsequence_len` prices
2. FFT the full `period` window and the reversed query; multiply spectra; IFFT to get all sliding dot products in O(N log N)
3. Convert dot products to z-normalized Euclidean distances via Pearson correlation: `dist² = 2m(1 - ρ)` where `ρ = QT / (m · std_i)`
4. Take the minimum distance across all non-degenerate candidates
5. Map to novelty: `value = 1 / (1 + min_dist)` (perfect match → near 0, novel → approaches 1)

## Parameters

| Parameter | Default | Constraint | Description |
|---|---|---|---|
| `period` | — | > 0 | Rolling window length (price samples) |
| `subsequence_len` | 30 | < period | Length of each subsequence compared |

## Output
- `value: f64` — novelty score ∈ [0, 1] (0 = perfect motif match, 1 = fully novel)

## Status
Implemented. Engine: `stomp.rs`. NT: `MatrixProfile`. Feature-gated: `signal-processing`.
