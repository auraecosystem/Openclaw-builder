# Signal Processing Methods for Noisy Timeseries

Methods for extracting patterns from semi-noisy timeseries data, organized by category. Source of the 18 algorithms implemented in `engine/crates/signals/`.

---

## 1. Frequency-Domain Methods

**Fourier Transform (FFT/DFT)** — Decomposes a signal into constituent frequencies. Great for identifying periodic/cyclical components. Limitation: assumes stationarity, so it loses temporal localization (you know which frequencies, but not when they occur).

**Short-Time Fourier Transform (STFT)** — Windowed FFT that provides time-frequency representation. Trade-off between time and frequency resolution (Heisenberg uncertainty).

**Spectral Analysis / Power Spectral Density** — Identifies dominant frequencies and their power. Useful for detecting hidden periodicities buried in noise.

---

## 2. Wavelet Methods

**Discrete Wavelet Transform (DWT)** — Multi-resolution decomposition that provides both time and frequency information. Unlike FFT, handles non-stationary signals well. Denoise by thresholding wavelet coefficients (hard/soft thresholding).

**Maximal Overlap DWT (MODWT)** — Translation-invariant variant. Particularly effective for high-frequency financial data denoising.

**Continuous Wavelet Transform (CWT)** — Provides a full scalogram; good for visual pattern identification across scales.

**Empirical Wavelet Transform (EWT)** — Constructs wavelets from the data's frequency content rather than using predefined basis functions.

---

## 3. Adaptive Decomposition Methods

**Empirical Mode Decomposition (EMD)** — Data-driven method that decomposes a signal into Intrinsic Mode Functions (IMFs), each representing dynamics at a different timescale. No predefined basis functions. Works on nonlinear, non-stationary data. Limitation: mode mixing (two components bleed into one IMF).

**EEMD / CEEMDAN / ICEEMDAN** — Ensemble variants that add noise to mitigate mode mixing. Strong combo for financial timeseries: decompose first, then denoise the noisy IMFs with wavelets.

**2LE-CEEMDAN** — Extension that automatically separates signal IMFs from noise IMFs.

**Variational Mode Decomposition (VMD)** — Optimization-based alternative to EMD. Decomposes modes concurrently rather than sequentially. More mathematically grounded, less prone to mode mixing.

**Singular Spectrum Analysis (SSA)** — Uses trajectory matrix + SVD to extract trend, oscillatory, and noise components. Model-free and nonparametric. Also effective for change-point detection by tracking subspace distances.

---

## 4. Filtering / Smoothing

**Kalman Filter** — Recursive Bayesian filter that maintains a state estimate and uncertainty, updating with each new observation. Optimal for linear systems with Gaussian noise. Variants: Extended Kalman Filter (EKF) for nonlinear systems, Unscented Kalman Filter (UKF) for highly nonlinear systems.

**Savitzky-Golay Filter** — Fits successive polynomials to local windows. Preserves higher moments (peaks, valleys) better than moving average while still smoothing.

**Moving Average / Exponential Smoothing** — Simple but effective low-pass filters. EMA weights recent data more heavily. Good baseline.

**Butterworth / Chebyshev Filters** — Classic IIR low-pass/band-pass filters with well-defined frequency cutoff characteristics.

**Hodrick-Prescott Filter** — Designed for economic timeseries; decomposes into trend + cyclical components via a smoothness penalty.

---

## 5. Statistical / Probabilistic Models

**Hidden Markov Models (HMM)** — Latent states with transition probabilities. Excellent for regime detection (bull/bear markets, volatility regimes). The Viterbi algorithm finds the most likely state sequence.

**Gaussian Processes (GP)** — Nonparametric Bayesian regression. Provides a posterior distribution over functions, giving uncertainty estimates alongside pattern extraction. Computationally expensive for long series.

**Bayesian Structural Time Series (BSTS)** — Decomposes into trend, seasonality, regression, with full posterior inference. Google's CausalImpact library uses this.

**Change-Point Detection (CUSUM, PELT, BOCPD)** — Algorithms that identify structural breaks. BOCPD (Bayesian Online Changepoint Detection) provides strong theoretical guarantees on detection delay.

---

## 6. Autocorrelation / Periodicity Detection

**Autocorrelation Function (ACF)** — Detects repeating patterns by measuring self-similarity at different lags. Peaks in the ACF indicate periodicity.

**Partial Autocorrelation (PACF)** — Isolates direct lag relationships, filtering out indirect ones.

**Periodogram / Lomb-Scargle** — Frequency-domain periodicity detection. Lomb-Scargle handles unevenly-sampled data.

---

## 7. Machine Learning Methods

**Denoising Autoencoders** — Learn nonlinear denoising mappings. Capture complex patterns that linear methods miss.

**Diffusion Model Denoisers** — Applies the diffusion/denoising framework (from image generation) to timeseries. More flexible than autoencoders due to iterative refinement.

**1D Convolutional Networks** — Learn local pattern templates (filters) directly from data. Essentially learn their own wavelet-like basis.

**Transformer / Attention Models** — Capture long-range dependencies without the vanishing gradient problem of RNNs. Models like PatchTST, TimesFM treat timeseries as sequences of patches.

**LSTM / GRU** — Recurrent architectures that maintain hidden state. Good for sequential pattern recognition in noisy data, though largely superseded by transformers for many tasks.

---

## 8. Matrix / Tensor Methods

**Dynamic Mode Decomposition (DMD)** — Extracts spatiotemporal coherent structures. Finds modes that evolve as e^(λt), identifying growth/decay/oscillation patterns.

**Matrix Profile** — Efficiently finds all nearest-neighbor subsequence distances. Reveals motifs (repeated patterns), discords (anomalies), and regimes. Very fast (STUMPY library).

**PCA / SVD** — On embedded timeseries, separates dominant patterns from noise via rank reduction.

---

## Practical Recommendations for Crypto Momentum Data

| Method | Strength | Use Case |
|---|---|---|
| VMD or CEEMDAN | Adaptive, no predefined basis | Decompose price into trend + swing + noise components |
| Wavelet thresholding | Clean frequency separation | Denoise before pattern detection |
| Kalman Filter | Recursive, real-time | Smooth price/indicator feeds adaptively |
| HMM | Regime detection | Identify bull/bear/ranging states |
| Matrix Profile | Motif discovery | Find recurring chart patterns |
| SSA | Trend + change-point | Detect structural breaks in momentum |

**Recommended pipeline**: decompose (EMD/VMD/SSA) → denoise individual components (wavelet threshold) → detect patterns/regimes (HMM/change-point) → validate (autocorrelation/statistical tests).

---

## Sources

- https://asp-eurasipjournals.springeropen.com/articles/10.1186/s13634-024-01115-5
- https://peerj.com/articles/cs-1852/
- https://www.nature.com/articles/s41598-023-28390-w
- https://arxiv.org/html/2409.02138v1
- https://arxiv.org/abs/2112.10139
- https://samanemami.medium.com/what-is-empirical-mode-decomposition-3ec89115db6b
- https://en.wikipedia.org/wiki/Kalman_filter
- https://en.wikipedia.org/wiki/Singular_spectrum_analysis
- https://proceedings.neurips.cc/paper/2021/file/c348616cd8a86ee661c7c98800678fad-Paper.pdf
- https://mitibmwatsonailab.mit.edu/research/blog/change-point-detection-via-multivariate-singular-spectrum-analysis/
