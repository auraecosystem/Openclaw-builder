# Wavelet Scattering Transform
Shift-invariant multi-scale feature extraction using cascaded Morlet wavelet convolutions and modulus operators. Produces stable, deformation-invariant representations of price windows. A pre-trained linear classifier (sigmoid of dot product with learned weights) converts scattering coefficients to a breakout probability in (0, 1).

## Use Cases
- Shift-invariant feature extraction robust to small timing variations in patterns
- Multi-scale volatility characterization across dyadic frequency bands
- Pre-breakout pattern recognition with learned classifier weights
- Feature input for downstream classifiers (SVM, logistic regression, gradient boosting)

## Limitations
- Classifier weights must be trained offline — requires labeled pre-breakout examples
- Untrained fallback (energy concentration heuristic) is a rough proxy, not a calibrated signal
- O(J × N log N) per bar where J = `j_max`; non-trivial at high frequency
- `FftPlanner` is re-created each call — reuse across bars for throughput
- Order-2 coefficient count is J(J-1)/2; can exceed weight vector length if `j_max` is large
- Not shift-equivariant (the full scattering network requires low-pass averaging per scale)

## Algorithm

**Order-1 scattering coefficients:**
```
For each scale j in 0..j_max:
  psi_j = Morlet wavelet at center freq pi / 2^j (Gaussian envelope in frequency domain)
  U1[j] = |IFFT(FFT(x) * psi_j)|    # modulus of filtered signal
  S1[j] = mean(U1[j])                # temporal average = order-1 coefficient
```

**Order-2 scattering coefficients:**
```
For each j1 < j2 in 0..j_max:
  S2[j1, j2] = mean(|IFFT(FFT(U1[j1]) * psi_j2)|)
```

Total coefficients: `j_max + j_max*(j_max-1)/2`. For `j_max=6`: 6 + 15 = 21 coefficients.

**Classifier:**
```
score = sigmoid(dot(coeffs, weights) + bias)
```
Fallback when `weights` is empty: `sigmoid(max(coeffs) / sum(coeffs))` (energy concentration ratio).

**Morlet spectrum** (frequency domain Gaussian):
```
psi_j(omega) = exp(-0.5 * (omega - omega0)^2 * sigma^2)
  where omega0 = pi / 2^j,  sigma = 1 / omega0
```

## Parameters

| Parameter | Default | Range | Description |
|---|---|---|---|
| `j_max` | 6 | 3–8 | Number of dyadic octave scales |
| `window` length | 64–128 | 32–256 | Input price window; power of 2 preferred |
| `weights` | `[]` | — | Pre-trained classifier weights (length = num coefficients) |
| `bias` | 0.0 | — | Classifier bias term |

## Complexity
Per bar: O(J × N log N) FFT convolutions + O(J² × N log N) order-2 terms.
For `j_max=6`, `N=64`: ~21 FFTs of size 128 — fast in practice with FFT caching.

## Output
- `score: f32` — breakout probability in (0, 1) via sigmoid classifier
- 0.5 returned for degenerate inputs (window too short, `j_max=0`)

## References
Mallat (2012) — "Group Invariant Scattering", CPAM
Bruna & Mallat (2013) — scattering convolution networks
LeCun group — deep scattering spectrum for audio/time-series

## Status
Implemented in engine only (`scattering.rs`). Classifier weights untrained — fallback heuristic active. Not ported to NautilusTrader. Tier 3.
