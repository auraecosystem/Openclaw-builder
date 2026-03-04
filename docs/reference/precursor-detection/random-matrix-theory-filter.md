# Random Matrix Theory (RMT) Correlation Filter
Marchenko-Pastur noise filtering on empirical correlation matrices. Eigendecomposition via faer, noise eigenvalues replaced with their mean, signal eigenvalues preserved. Produces a denoised correlation matrix and factor count.

## Use Cases
- Filtered correlation for portfolio construction: removes noise-driven co-movements from N-asset correlation matrices
- Factor counting: `n_factors` = eigenvalues above the Marchenko-Pastur upper bound — identifies genuine common drivers
- Regime detection: rising `n_factors` indicates increasing market co-movement (stress / correlation breakdown)

## Limitations
- Multi-asset: takes a flat `(n_tickers × n_bars)` returns matrix — not a single-series indicator
- Not ported to NT; operates at the portfolio/cross-sectional level, not per-instrument
- Marchenko-Pastur bound assumes i.i.d. Gaussian returns; fat-tailed or autocorrelated returns shift the bound
- Constant-variance tickers are rejected (standardization step returns None)

## Algorithm

1. Standardize each ticker's returns to zero mean and unit variance
2. Compute empirical correlation matrix `C = (1/T) · X · Xᵀ` (shape N×N)
3. Eigendecompose C via faer self-adjoint solver (lower triangle)
4. Apply Marchenko-Pastur bounds: `λ± = (1 ± √(N/T))²`; eigenvalues in `[λ−, λ+]` are noise
5. Replace noise eigenvalues with their mean; preserve signal eigenvalues above `λ+`
6. Reconstruct `C_filtered = U · diag(λ_filtered) · Uᵀ`; normalize diagonal to 1

## Parameters

| Parameter | Description |
|---|---|
| `n_tickers` | Number of assets in the returns matrix |
| `n_bars` | Number of time steps per asset (T in MP theory) |

No period parameter — operates on the full provided matrix.

## Output
- `filtered_corr: Vec<f32>` — row-major N×N filtered correlation matrix
- `top_eigenvalues: Vec<f32>` — signal eigenvalues (above MP upper bound)
- `n_factors: usize` — number of significant common factors

## Status
Implemented in engine only (`rmt.rs`). Not ported to NT (requires multi-asset matrix input).
