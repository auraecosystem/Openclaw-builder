# Transfer Entropy (KSG Estimator)
Information flow from source to target series using Kraskov-Stögbauer-Grassberger KNN estimator. Measures directed information transfer — high TE from volume to price means volume "leads" price.

## Use Cases
- Lead-lag detection: identify which asset or indicator Granger-causes another in an information-theoretic sense
- Volume → price coupling: TE(volume→price) rising before breakouts is a confirmed precursor signal
- Cross-asset causality: BTC → altcoin information flow strengthens during correlated regimes

## Limitations
- Multi-asset: requires two aligned series of equal length — does not fit the single-series NT Indicator trait
- O(N²) neighbor counting in marginal spaces; scales poorly beyond N=1000
- KSG estimator is asymptotically unbiased but biased on small N — use N ≥ 100 per series
- `lag` capped at 4 due to compile-time const-generic KD-tree dimension (joint dim = 3·lag)

## Algorithm

Joint embedding: `X = target_past(lag)`, `Y = source_past(lag)`, `Z = target_future`. Total dim = 3·lag.

1. Build KD-tree in joint (X, Y, Z) space; find Chebyshev k-th neighbor distance `ε_i` for each point
2. Count neighbors within `ε_i` in three marginal spaces: (X,Z), (X,Y), (X)
3. TE = `ψ(k) - mean(ψ(n_XZ+1) + ψ(n_XY+1) - ψ(n_X+1))`  where ψ is the digamma function

## Parameters

| Parameter | Default | Description |
|---|---|---|
| `k` | 5 | Number of nearest neighbors for KSG estimator |
| `lag` | 1 | Embedding lag (1–4; capped by const-generic KD-tree dimension) |

No `period` parameter — operates on the full provided series slice.

## Output
- `f32` — TE ≥ 0; higher = stronger directed information flow from source to target

## Status
Implemented in engine only (`transfer.rs`). Not ported to NT (requires two input series).
