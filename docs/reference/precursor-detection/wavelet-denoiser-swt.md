# Wavelet Denoiser (SWT)
Stationary Wavelet Transform via à-trous algorithm with Sym8 filter + universal soft-thresholding (VisuShrink). Returns denoised price. Unlike FFT filters, SWT preserves signal length at every scale and avoids circular boundary artefacts.

## Use Cases
- Noise-free price series for downstream pattern and signal detection
- Separating genuine price structure from tick noise
- Pre-processing before Hurst, PE, or matrix profile computation

## Limitations
- VisuShrink threshold is aggressive: `σ·√(2·ln N)` — may over-smooth fast-moving breakouts
- Noise estimate (MAD on first detail band) assumes Gaussian noise; financial noise has fat tails
- Computationally heavier than EMA or Kalman for the same smoothing effect

## Algorithm

Sym8 16-tap scaling filter applied at each level with dyadic stride `2^level` (à-trous / "with holes" trick — no downsampling, output length unchanged).

1. **Decompose**: at each level, compute `detail = approx - lowpass(approx)` using the Sym8 filter with reflect boundary extension
2. **Noise estimate**: median absolute deviation of level-0 details, normalized by 0.6745 (Gaussian consistency factor)
3. **Threshold**: universal soft threshold `σ·√(2·ln N)` applied to all detail bands
4. **Reconstruct**: sum thresholded detail bands back onto the coarsest approximation

## Parameters

| Parameter | Default | Description |
|---|---|---|
| `period` | — | Rolling window of close prices |
| `level` | 3 | Number of SWT decomposition levels |

## Output
- `value: f64` — denoised close price at the latest bar

## Status
Implemented. Engine: `swt.rs`. NT: `WaveletDenoiser`.
