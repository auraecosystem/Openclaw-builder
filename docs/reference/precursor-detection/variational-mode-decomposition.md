# Variational Mode Decomposition (VMD)
FFT-based VMD via ADMM + STA/LTA ratio on swing mode. Decomposes price into K frequency modes (trend, swing, noise). STA/LTA expanding = breakout, compressing = coiling.

## Use Cases
- Transient regime detection: rising STA/LTA on the swing mode signals an emerging structural break
- Coiling detection: low, stable STA/LTA indicates energy compression before a move
- Multi-scale decomposition: separates trend, swing, and noise components for independent analysis

## Limitations
- Computationally expensive: O(N log N) FFT per update, plus `n_iter` ADMM iterations
- `alpha` (bandwidth) must be tuned: too small = overlapping modes, too large = rigid decomposition
- Zero-pads to next power of two — introduces boundary effects at window edges
- `tau=0` (default) disables the Lagrangian dual update; non-zero tau tightens mode fidelity but may not converge

## Algorithm

Dragomiretskiy & Zosso (2014) ADMM formulation in the frequency domain:

1. Zero-pad signal to next power of two, compute FFT
2. Initialize K mode center frequencies evenly across [0, 0.5]
3. For each ADMM iteration, for each mode: subtract other modes' FFT contributions (residual), apply Wiener filter with bandwidth `alpha` around center frequency; update center frequency as power-weighted mean of positive spectrum
4. Select the mid-frequency "swing" mode (index `k_modes//2` after sorting by center frequency)
5. IFFT the swing mode back to time domain; compute STA/LTA of instantaneous amplitude

## Parameters

| Parameter | Default | Description |
|---|---|---|
| `period` | — | Rolling window of close prices |
| `k_modes` | 3 | Number of intrinsic mode functions to extract |
| `alpha` | 2000.0 | ADMM bandwidth constraint (larger = narrower modes) |
| `sta_window` | 10 | Short-term averaging window for STA/LTA |
| `lta_window` | 50 | Long-term averaging window for STA/LTA |
| `n_iter` | 15 | ADMM iteration count |
| `tau` | 0.0 | Lagrangian multiplier step size (0 = no dual update) |

## Output
- `value: f64` — STA/LTA ratio of swing mode amplitude (≥ 0; > 1 = expanding, < 1 = compressing)

## Status
Implemented. Engine: `vmd.rs`. NT: `VariationalModeDecomposition`. Feature-gated: `signal-processing`.
