# Agent Report: Precursor Detection — Remaining Algorithm Docs

## Task
Create 3 algorithm reference docs for engine-only algorithms plus 2 special index files in `/Users/ad/work/ai/openclaw/docs/reference/precursor-detection/`.

## Files Created

### Algorithm Docs (Task 1)

**`/Users/ad/work/ai/openclaw/docs/reference/precursor-detection/knn-anomaly-detection.md`**
- Read `knn_anomaly.rs`: brute-force O(N×dim) KNN with `select_nth_unstable_by` partial sort; KdTree variant via `kiddo` for compile-time DIM=10
- Key insight from source: brute-force is preferred for N < ~1000 because tree rebuild cost per bar exceeds the linear scan
- Documented the unnormalized output (caller normalizes offline against a baseline)

**`/Users/ad/work/ai/openclaw/docs/reference/precursor-detection/wavelet-scattering-transform.md`**
- Read `scattering.rs`: Morlet wavelet spectra in frequency domain, order-1 and order-2 scattering coefficients, sigmoid classifier
- Key detail: `FftPlanner` is re-created each call (noted as a throughput optimization opportunity)
- Fallback when weights are empty: energy concentration ratio `max(coeffs)/sum(coeffs)` — documented as a rough heuristic, not calibrated
- Coefficient count formula: `j_max + j_max*(j_max-1)/2`; for j_max=6 that is 21 coefficients

**`/Users/ad/work/ai/openclaw/docs/reference/precursor-detection/template-matching.md`**
- Read `template.rs`: z-normalize both window and template, FFT cross-correlation via reversal trick (convolution = correlation with reversal), normalize by `min(len_a, len_b)`
- Noted the LIGO matched-filtering lineage from the module docstring
- Edge case documented: returns 0.0 for constant windows (std < 1e-10) and empty template banks

### Special Files (Tasks 2 & 3)

**`/Users/ad/work/ai/openclaw/docs/reference/precursor-detection/_ml-framework-methods.md`**
- Verbatim from spec; underscore prefix sorts to top in most file listings

**`/Users/ad/work/ai/openclaw/docs/reference/precursor-detection/_index.md`**
- Full index with tier table, implemented/to-implement/not-feasible sections
- All links verified against actual filenames in the directory

## Verification

```
ls docs/reference/precursor-detection/ | grep -E '(knn|scattering|template|_index|_ml)'
_index.md
_ml-framework-methods.md
knn-anomaly-detection.md
template-matching.md
wavelet-scattering-transform.md
```

Total files in directory after task: 35 (was 30).

## Adaptations
- None required. All source files existed at expected paths. Doc structure followed the established pattern from `conformal-anomaly-detection.md`.
- Scattering doc notes the untrained classifier state (fallback heuristic active) — this is accurate per the source code's `if weights.is_empty()` branch.
- Template matching doc notes the `FftPlanner` re-creation issue per the source — relevant for future optimization.
