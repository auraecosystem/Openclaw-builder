# Empirical Mode Decomposition (EMD)
Data-driven signal decomposition into Intrinsic Mode Functions (IMFs). Unlike wavelets/Fourier, EMD is adaptive — it derives basis functions from the signal itself. Decomposes nonstationary series into trend, oscillatory modes, and noise without assuming stationarity.

## Use Cases
- Multi-scale precursor extraction (each IMF captures a different time scale)
- Denoising via IMF selection (discard high-frequency IMFs)
- Trend extraction without parametric assumptions
- Complement to VMD: fully adaptive vs optimization-based decomposition

## Limitations
- Mode mixing: single IMF may contain multiple scales
- Boundary effects from spline interpolation
- Sensitivity to irregular sampling
- Not mathematically rigorous (no formal basis, no convergence proof)
- EEMD addresses mode mixing but multiplies compute by ensemble size

## Algorithm

**Sifting (extract one IMF from residual r(t)):**
1. Find local maxima and minima of r(t)
2. Cubic spline upper envelope e_upper through maxima
3. Cubic spline lower envelope e_lower through minima
4. Mean envelope: `m(t) = (e_upper + e_lower) / 2`
5. Candidate: `h(t) = r(t) - m(t)`
6. If not converged: r(t) <- h(t), go to 1
7. Output IMF_k = h(t), then r(t) <- r(t) - IMF_k, extract next

**Stopping:** Cauchy SD `= Σ|h_{k-1} - h_k|² / h_{k-1}² < threshold`.

**EEMD:** Add noise `w_i ~ N(0, ε²)` to signal, EMD each, average IMFs across ensemble.

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Window | `period` | 256 | 64–1024 | Rolling window |
| Max siftings | `max_siftings` | 100 | 10–500 | Per-IMF limit |
| Sift threshold | `sift_threshold` | 0.2 | 0.01–0.5 | Cauchy SD convergence |
| Max IMFs | `max_imfs` | 8 | 2–floor(log2(n)) | Mode count limit |
| Max energy ratio | `max_energy_ratio` | 20.0 | 10–40 | Residual cutoff (dB) |
| Noise amplitude | `noise_amplitude` | 0.2×std | 0.05–0.4×std | EEMD noise level |
| Ensemble size | `n_ensemble` | 200 | 50–500 | EEMD repetitions |
| Envelope method | `envelope` | cubic_spline | cubic_spline/pchip | Interpolation |

## Complexity
EMD: O(K × S × N log N). EEMD: O(N_e × K × S × N log N).

## Output
- `value: f64` — IMF₁ energy / total energy
- `imfs: Vec<Vec<f64>>`, `residual: Vec<f64>`, `n_imfs: usize`

## References
Huang et al. (1998) Proc. Royal Society A 454; Wu & Huang (2009) EEMD

## Status
To implement. Zero deps for basic EMD; EEMD is CPU-heavy. Tier 3.
