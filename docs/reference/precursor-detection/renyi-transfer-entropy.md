# Rényi Transfer Entropy
Alpha-parameterized TE for tail-regime coupling. α=2 standard coupling; α=5 emphasizes tails, detecting contagion and stress propagation. KNN-based Rényi entropy estimation via Leonenko-Pronzato-Savani estimator.

## Use Cases
- Tail-regime coupling (α=5): detects contagion — when stress in one asset causes sharp co-movement in another
- Standard coupling (α=2): equivalent to standard information-theoretic TE but in Rényi generalization
- Regime-specific causality: run at multiple α values; divergence between α=2 and α=5 signals tail-driven rather than mean-driven coupling

## Limitations
- Multi-asset: requires two aligned series — not ported to NT single-series Indicator trait
- α=1 is undefined (Shannon limit); returns 0.0
- Brute-force O(N²) KNN; not suitable for large N (> 500 points per embedding)
- Rényi TE differences can be negative due to numerical noise; clamped to ≥ 0

## Algorithm

RTE = H_α(target_future | target_past) − H_α(target_future | target_past, source_past)

Using chain rule: `RTE = H(joint_ts) − H(cond_ts) − H(marginal) + H(cond_t)`

Where embeddings are:
- `joint_ts`: (target_future, target_past(lag), source_past(lag))
- `cond_ts`: (target_past(lag), source_past(lag))
- `marginal`: (target_future, target_past(lag))
- `cond_t`: (target_past(lag))

Rényi entropy per space: `H_α = 1/(1-α) · ln( mean(ε_i^(d·(1-α))) )` where ε_i = k-th NN distance. Brute-force k-NN scan used for small N; kiddo KD-tree path reserved for larger datasets.

## Parameters

| Parameter | Default | Description |
|---|---|---|
| `alpha` | 2.0 | Rényi order (2.0 = standard, 5.0 = tail-sensitive; must ≠ 1.0) |
| `k` | 5 | Number of nearest neighbors for entropy estimation |
| `lag` | 1 | Delay embedding lag |

## Output
- `f32` — RTE ≥ 0

## Status
Implemented in engine only (`renyi.rs`). Not ported to NT (requires two input series).
