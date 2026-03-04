# Template Matching
FFT cross-correlation of the current price window against a bank of known consolidation templates. Borrowed from gravitational wave detection (LIGO matched filtering): the method finds the best-fitting known pattern shape regardless of amplitude or mean level. Returns the maximum Pearson correlation across all templates — close to 1.0 means the current price action closely resembles a known pre-breakout shape.

## Use Cases
- Pattern recognition via explicit template library (VCP, flag, cup-with-handle shapes)
- Amplitude- and shift-invariant similarity to known pre-breakout structures
- Complement to learned detectors (VCP/flag) where template shapes are known analytically
- Detection of recurring structural regimes without parametric models

## Limitations
- Template bank must be curated manually or extracted from historical labeled examples
- All-or-nothing: only matches templates in the bank; novel patterns score near zero
- FFT cross-correlation gives circular correlation — zero-padding prevents aliasing but window length affects lag range
- Z-normalization removes amplitude information; pure shape matching only
- O(T × N log N) per bar where T = number of templates
- Pearson correlation threshold (e.g. > 0.7) must be chosen per template bank

## Algorithm

**Z-normalization:**
```
w_norm = (window - mean(window)) / std(window)
t_norm = (template - mean(template)) / std(template)
```

**FFT cross-correlation:**
```
fft_len = next_power_of_two(len(window) + len(template))
A = FFT(w_norm, zero-padded to fft_len)
B = FFT(reverse(t_norm), zero-padded to fft_len)   # reversal → convolution = correlation
corr_t = IFFT(A * B) / fft_len
max_corr_t = max(corr_t.re) / min(len(window), len(template))
```

The normalization by `min(len(window), len(template))` converts raw cross-correlation energy to a Pearson-like statistic. Because both signals are z-normalized, this approximates the peak Pearson correlation over all lags.

**Best match:**
```
score = max(max_corr_t for template t in bank)
```
Clamped to [-1, 1]. Returns 0.0 if no valid template or constant window.

## Parameters

| Parameter | Default | Range | Description |
|---|---|---|---|
| `window` length | 50 | 20–200 | Current price window length |
| Template bank size | — | 1–50 | Number of template shapes |
| Template length | — | ≤ window | Each template; same or shorter than window |
| Match threshold | 0.7 | 0.5–0.95 | Correlation threshold for signal |

## Complexity
Per bar: O(T × N log N) where T = template count, N = `fft_len` ≈ 2×window.
For T=10, N=128: ~10 FFT triples — negligible.

## Output
- `score: f32` — maximum Pearson correlation in [-1, 1] across all templates
- 1.0 = exact shape match; 0.0 = no resemblance or empty bank; -1.0 = inverse shape

## References
Allen (2005) — LIGO matched filtering, Phys. Rev. D 85, 122006
Keogh & Ratanamahatana (2005) — exact indexing of dynamic time warping (template context)
Rakthanmanon et al. (2012) — UCR Suite: scaling up time series similarity search

## Status
Implemented in engine only (`template.rs`). Template bank not yet populated from labeled data. Not ported to NautilusTrader. Tier 2.
