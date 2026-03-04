# Precursor Detection Algorithm Reference

Per-concept reference docs for all algorithms from the cross-disciplinary precursor detection survey. Each file is self-contained with use cases, limitations, exact math, parameters, complexity, and implementation status.

## Implementation Tiers

| Tier | Complexity | Algorithms |
|---|---|---|
| 1 | O(1) per bar, zero deps | CUSUM, EWMA, Concept Drift, Robust Dispersion |
| 2 | O(n) to O(n log n) | Quickest Change AR, First-Passage Time, Survival Model, Conformal Anomaly, Scan Statistics |
| 3 | O(n²) or batch-heavy | ApEn, SampEn, RQA, Visibility Graph, EMD, PELT, L1 Trend Filtering |
| 4 | Specialized pattern mining | DTW Motif + MDL, Collective Anomaly/Discord |

## Already Implemented

### In NautilusTrader (Rust indicators)
- [Permutation Entropy](permutation-entropy.md) — ordinal complexity
- [BOCPD](bayesian-changepoint-detection.md) — changepoint probability
- [Kalman Trend Filter](kalman-trend-filter.md) — zero-lag trend + velocity
- [Hurst Exponent](hurst-exponent-dfa.md) — long-range dependence (DFA)
- [HMM Regime](hidden-markov-regime.md) — 3-state Viterbi decoder
- [VPIN](volume-clock-vpin.md) — informed trading probability
- [Wavelet Denoiser](wavelet-denoiser-swt.md) — SWT à-trous denoising
- [VMD](variational-mode-decomposition.md) — frequency decomposition + STA/LTA
- [Matrix Profile](matrix-profile-stomp.md) — STOMP/MASS novelty

### In Engine Only (not ported to NT)
- [Conformal Prediction](conformal-anomaly-detection.md) — calibrated p-values
- [Transfer Entropy](transfer-entropy.md) — directed information flow (multi-asset)
- [Rényi TE](renyi-transfer-entropy.md) — tail-regime coupling (multi-asset)
- [RMT Filter](random-matrix-theory-filter.md) — correlation noise removal (multi-asset)
- [KNN Anomaly](knn-anomaly-detection.md) — distance-based anomaly scoring
- [Scattering Transform](wavelet-scattering-transform.md) — multi-scale invariant features
- [Template Matching](template-matching.md) — FFT cross-correlation

## To Implement

### Tier 1 (zero deps, O(1) per bar)
- [CUSUM](cusum-control-chart.md) — cumulative sum change detection
- [EWMA](ewma-control-chart.md) — exponential smoothing with control limits
- [Robust Dispersion](robust-dispersion-contraction.md) — MAD/IQR contraction ratio
- [Concept Drift](concept-drift-detection.md) — DDM / Page-Hinkley / ADWIN

### Tier 2 (moderate complexity)
- [Quickest Change AR](quickest-change-ar.md) — CUSUM on AR residuals
- [First-Passage Time](first-passage-time.md) — hitting-time probability
- [Survival Model](survival-hazard-model.md) — Kaplan-Meier / Nelson-Aalen
- [Scan Statistics](scan-statistics.md) — sliding-window LLR surveillance

### Tier 3 (computationally heavy)
- [ApEn](approximate-entropy.md) — approximate entropy (O(N²))
- [SampEn](sample-entropy.md) — sample entropy (O(N²))
- [RQA](recurrence-quantification-analysis.md) — recurrence quantification (O(N²))
- [Visibility Graph](visibility-graph.md) — time-series graph topology
- [EMD](empirical-mode-decomposition.md) — empirical mode decomposition
- [PELT](pelt-changepoint-detection.md) — optimal offline changepoint detection
- [L1 Trend Filtering](l1-trend-filtering.md) — ADMM piecewise-polynomial

### Tier 4 (specialized)
- [DTW Motif + MDL](dtw-motif-discovery.md) — warping-invariant pattern discovery
- [Collective Anomaly](collective-anomaly-discord.md) — HOT SAX discord detection

## Meta-Optimization (Parameter & Formula Search)
- [Evolutionary Parameter Optimization](evolutionary-parameter-optimization.md) — CMA-ES, JADE/DE, NSGA-II/MOEA/D, Grammar-guided GP, Surrogate-assisted, MAP-Elites/CMA-MAE

## Not Feasible as Rust Indicators
- [ML Framework Methods](_ml-framework-methods.md) — transformers, diffusion, GNNs, foundation models, etc.
