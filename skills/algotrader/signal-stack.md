# Signal Extraction Stack — Complete Algorithm Reference

21 algorithms across 4 layers. The architecture is a funnel: cheap/fast functions run on every bar across all tickers (high recall), progressively more expensive functions run only on candidates (high precision), and an LLM agent makes the final go/no-go decision.

```
100 tickers × 1 bar (every 5 min)
        │
   ┌────┴────┐
   │ LAYER 0 │  Data characterization (per-asset, every ~100 bars)
   └────┬────┘
        │
   ┌────┴────┐
   │ LAYER 1 │  Scanner — 10 functions, every bar, every ticker
   └────┬────┘
        │  composite score > threshold → wake agent
   ┌────┴────┐
   │ LAYER 2 │  Agent investigation — 3 functions, only on candidates
   └────┬────┘
        │  ENTER / SKIP / WATCHLIST
   ┌────┴────┐
   │ LAYER 3 │  Execution — 4 functions, only on open positions
   └─────────┘
```

---

## Layer 0 — Data Characterization

Updated every ~100 bars per asset. Sets the regime context that conditions all downstream decisions.

### 0.1 Rolling Hurst Exponent (DFA)

| Field | Value |
|-------|-------|
| **Input** | Trailing 500 bars of log returns |
| **Output** | H ∈ [0, 1] |
| **Compute** | O(n log n), every 50-100 bars |
| **Library** | `nolds.dfa()` or `MFDFA` |

**How to use:**
- H > 0.55 → trending regime → use breakout/momentum strategies
- H < 0.45 → mean-reverting regime → use fade strategies or stay out
- 0.45 < H < 0.55 → random walk → reduce position sizes, tighten filters

**Why:** The Hurst exponent tells you whether the market has memory. Momentum strategies fail in mean-reverting regimes. This is the single most important regime filter.

---

### 0.2 HMM Regime State

| Field | Value |
|-------|-------|
| **Input** | Trailing 1000+ bars of returns + volume |
| **Output** | Discrete state (e.g., 0=low-vol, 1=high-vol, 2=trending) |
| **Compute** | O(n) inference, fit offline on training data |
| **Library** | `hmmlearn.GaussianHMM(n_components=3)` |

**How to use:**
- Fit 2-3 state HMM on daily returns (offline, on training data)
- At runtime, decode current state with Viterbi
- Condition sub-strategy selection: breakout scanner active only in states where breakouts historically succeed
- Monitor state transition probabilities: P(trending→choppy) rising = tighten stops

**Why:** HMMs capture regime structure that simple threshold rules miss. Fit on daily data, condition intraday strategies.

---

### 0.3 RMT Correlation Filtering

| Field | Value |
|-------|-------|
| **Input** | Returns matrix (N assets × T bars), trailing 500+ bars |
| **Output** | Filtered correlation matrix, dominant eigenvectors (sector factors) |
| **Compute** | O(N²T), every 100-500 bars |
| **Library** | `numpy.linalg.eigh` + Marchenko-Pastur filter |

**How to use:**
1. Compute empirical correlation matrix from trailing returns
2. Eigendecompose. Compare eigenvalues to Marchenko-Pastur distribution for your N/T ratio
3. Eigenvalues within MP bounds = noise → replace with average eigenvalue
4. Eigenvalues outside MP bounds = signal → keep
5. Reconstruct filtered correlation matrix
6. Top eigenvector = market factor (beta). Next 2-5 = sector factors (DeFi, L1s, memecoins, etc.)

**Why:** The raw 100×100 correlation matrix is extremely noisy on 5m data. RMT filtering isolates genuine factor structure from noise. Use for: universe grouping, pair selection, risk decomposition.

---

### 0.4 Transfer Entropy Network

| Field | Value |
|-------|-------|
| **Input** | Returns for top 20 assets by volume, trailing 200+ bars |
| **Output** | Directed graph: TE(A→B) for all pairs, identifying information leaders |
| **Compute** | O(n²) per pair → O(N² n²) total, every 100 bars on top 20 |
| **Library** | `PyCausality` or `dit` |

**How to use:**
- Build a directed information flow graph: edge weight = TE(source→target)
- Identify current information leaders (high out-degree, high total outflow)
- Breakouts in information leaders are more likely to follow through (the leader is setting the direction)
- Breakouts in information followers while the leader is flat = lower conviction

**Why:** In crypto, BTC usually leads, but leadership rotates. During altcoin seasons, ETH or a sector leader takes over. TE detects this rotation in real time.

---

## Layer 1 — Scanner

Runs every bar (every 5 minutes) across all 100 tickers. Each function produces a normalized score. Combined into a composite score that determines whether to wake the LLM agent.

### 1.1 Permutation Entropy (PE) — Complexity Filter

| Field | Value |
|-------|-------|
| **Input** | Trailing W=30 bars of close prices |
| **Output** | PE ∈ [0, 1] where 0=perfectly ordered, 1=random |
| **Compute** | O(W), trivially fast |
| **Library** | `ordpy` or manual (~15 lines) |
| **Params** | `embedding_dim=5, delay=1, window=30` |

**How to use:**
- Compute PE on a rolling window every bar
- Track PE trajectory over last N=10 windows
- **DECLINING PE** over N consecutive windows = "this asset is coiling / consolidating"
- PE < 0.85 (rough threshold, tune on data) = "structured enough to be tradeable"
- PE spike after sustained decline = "breakout initiated"

**Signal output:**
- `pe_value`: current PE
- `pe_declining`: boolean, has PE been declining for >= 5 consecutive windows
- `pe_delta`: rate of PE change (negative = increasingly ordered)

**Why:** This is the cheapest, fastest pre-breakout detector. A consolidation is, by definition, more ordered than normal price action. PE measures exactly this.

---

### 1.2 BOCPD — Regime Shift Detector

| Field | Value |
|-------|-------|
| **Input** | Streaming bar returns, one at a time |
| **Output** | P(changepoint at current bar) ∈ [0, 1] |
| **Compute** | O(1) per bar (online, recursive) |
| **Library** | `changepoint` (Rust-backed) or `bocd` |
| **Params** | `hazard_rate=1/100, likelihood=StudentT(df=5)` |

**How to use:**
- Feed each bar's return into the BOCPD updater
- Track P(cp) at each bar
- **Two types of changepoint matter:**
  1. Volatile → quiet transition: "consolidation started" → start watching this asset
  2. Quiet → volatile transition: "breakout initiated" → entry trigger
- P(cp) > 0.5 → alert. P(cp) > 0.7 → high confidence regime shift.

**Signal output:**
- `cp_prob`: P(changepoint at this bar)
- `run_length`: estimated bars since last changepoint (how long the current regime has lasted)
- `cp_type`: inferred direction (volatility increasing or decreasing)

**Why:** BOCPD is Bayesian, online, handles fat tails (Student-t likelihood), and gives a probability rather than a binary. It detects both the START of consolidation and the breakout.

---

### 1.3 Kalman Filter — Zero-Lag Trend Smoother

| Field | Value |
|-------|-------|
| **Input** | Streaming close prices |
| **Output** | Smoothed price, trend velocity, trend acceleration |
| **Compute** | O(1) per bar |
| **Library** | `filterpy.KalmanFilter` or ~30 lines manual |
| **Params** | `Q` (process noise), `R` (measurement noise) — tune via EM on training data |

**How to use:**
- State vector: [price, velocity]. Transition: price += velocity. Observation: noisy close price.
- Q controls responsiveness: higher Q = tracks price more closely (noisier). Lower Q = smoother (more lag).
- R controls trust in observations: higher R = less trust in each bar (smoother).
- **Replace all SMA/EMA usage with Kalman-smoothed price.** It is mathematically optimal for this task.
- Velocity > 0 and acceleration > 0 = "trend strengthening"
- Velocity ≈ 0 = "flat / consolidating"

**Signal output:**
- `kf_price`: smoothed price (zero-lag estimate of true price)
- `kf_velocity`: instantaneous trend direction and strength
- `kf_acceleration`: trend momentum (is the trend speeding up or slowing down)

**Why:** Moving averages have inherent lag proportional to their period. Kalman filters don't — they're the minimum-variance linear estimator. On noisy 5m candles, this difference matters.

---

### 1.4 VMD + Inverse STA/LTA — Consolidation & Breakout Detector

| Field | Value |
|-------|-------|
| **Input** | Trailing W=100-200 bars of close prices |
| **Output** | STA/LTA ratio per VMD mode; compression score; breakout trigger |
| **Compute** | O(n log n) for VMD, run every 10-20 bars (not every bar) |
| **Library** | `vmdpy` + manual STA/LTA (~5 lines) |
| **Params** | `K=3` (modes), `alpha=2000` (VMD bandwidth), `STA_window=10`, `LTA_window=50` |

**How to use:**
1. Decompose trailing 200 bars via VMD into K=3 modes: trend (low freq), swing (mid freq), noise (high freq)
2. On the **swing mode**: compute STA(10) / LTA(50)
3. **Dropping** STA/LTA on swing mode = volatility compression = "coiling" (seismic quiescence analogy)
4. **Spiking** STA/LTA on swing mode = breakout (energy release)
5. Same indicator gives both the SETUP detection (inverse) and the ENTRY trigger (spike)

**Signal output:**
- `vmd_compression`: 1.0 - (STA/LTA on swing mode), clamped [0, 1]. High = compressed = coiling.
- `vmd_breakout`: boolean, STA/LTA crosses above 2.0 threshold
- `vmd_modes`: the 3 decomposed modes (for optional downstream use)

**Why:** VMD separates the consolidation signal from trend and noise. STA/LTA on the clean swing mode detects both compression (setup) and expansion (entry). Borrowed from seismology's "seismic quiescence" concept.

---

### 1.5 NPLM Background Anomaly Score — "Is This Unusual?"

| Field | Value |
|-------|-------|
| **Input** | Feature vector of trailing W=50 bars: [returns_std, range_pct, PE, volume_ratio, kf_velocity_std, autocorr_lag1, ...] (~10-15 features |
| **Output** | Anomaly score ∈ [0, ∞) where higher = more unusual |
| **Compute** | O(1) per window after training |
| **Library** | `sklearn.ensemble.IsolationForest` or PyTorch density estimator |
| **Params** | `contamination=0.05` (expected fraction of anomalies), `n_estimators=200` |

**How to use:**
1. **Training (offline):** Extract feature vectors from 100k random 50-bar windows in training data. Fit an Isolation Forest (or Gaussian Mixture Model, or normalizing flow).
2. **Runtime:** Extract same features from current 50-bar window. Score against the background model.
3. High anomaly score = "this window doesn't look like normal price action"
4. **The key insight: a consolidation IS the anomaly.** Normal crypto = volatile, noisy, high-entropy. A tightening base = abnormally quiet, structured, low-entropy. The model flags it without being told what a consolidation looks like.

**Signal output:**
- `nplm_score`: anomaly score (higher = more unusual)
- `nplm_percentile`: percentile rank vs. historical scores (e.g., "this is in the top 3% most unusual windows")

**Why:** This is model-agnostic and catches novel consolidation patterns you haven't templated. It's the "unknown unknowns" detector.

---

### 1.6 Wavelet Scattering Transform — Shift-Invariant Feature Extractor + Classifier

| Field | Value |
|-------|-------|
| **Input** | Trailing W=64 bars of close prices (power of 2 for efficiency) |
| **Output** | P(pre-breakout), P(random-chop), P(already-trending) |
| **Compute** | O(n) for features, O(1) for classification |
| **Library** | `kymatio` (scattering) + `xgboost` (classifier) |
| **Params** | `J=6` (octaves), `Q=1` (quality), classifier trained on ~100+ labeled windows |

**How to use:**
1. **Feature extraction:** Compute scattering coefficients at orders 1 and 2. These are shift-invariant (doesn't matter where in the window the pattern starts) and low-variance (stable across noise).
2. **Classification:** Feed scattering coefficients to a pre-trained XGBoost classifier.
3. **Label scheme (for training):**
   - "pre-breakout": windows where the subsequent 10-20 bars saw a move > 2% with volume confirmation
   - "random-chop": windows followed by flat/noisy action
   - "already-trending": windows already in the middle of a move
4. At runtime, the classifier outputs probabilities for each class.

**Signal output:**
- `wst_prebreakout_prob`: P(this is a pre-breakout consolidation)
- `wst_features`: raw scattering coefficient vector (for optional agent inspection)

**Why:** Scattering transform features have theoretical guarantees (Lipschitz continuity to deformations, invariance to translation). Small labeled dataset needed. Deterministic feature extraction = reproducible.

---

### 1.7 VPIN — Order Flow Quality Filter

| Field | Value |
|-------|-------|
| **Input** | Trade-level tick data, bucketed by equal volume |
| **Output** | VPIN ∈ [0, 1] where higher = more informed trading |
| **Compute** | O(n) on tick stream |
| **Library** | Manual (~40 lines) or `github.com/yt-feng/VPIN` |
| **Params** | `bucket_volume` (volume per bucket), `n_buckets=50` (estimation window) |

**How to use:**
1. Aggregate trades into equal-volume buckets (volume-time, not clock-time)
2. Classify each trade as buy or sell (tick rule or Lee-Ready)
3. Per bucket: buy_volume and sell_volume
4. VPIN = mean(|buy_vol - sell_vol| / total_vol) over last n_buckets

**Signal output:**
- `vpin_value`: current VPIN estimate
- `vpin_elevated`: boolean, VPIN > historical 75th percentile

**Use as a FILTER, not a signal:**
- Scanner fires + VPIN elevated = "informed traders are positioning" = HIGH-quality setup
- Scanner fires + VPIN low = "noise-driven" = LOWER-quality setup (still may be valid, but lower conviction)

**Why:** A 2025 study showed VPIN significantly predicts Bitcoin price jumps. Informed flow before a breakout = real move. No informed flow = more likely a fakeout.

**Requires:** Tick-level data feed from exchange (Binance WebSocket trade stream). Not available from daily/minute OHLCV bars alone.

---

### 1.8 Matrix Profile Motif Matching — "Looks Like a Known Setup"

| Field | Value |
|-------|-------|
| **Input** | Trailing W=200-500 bars (reference), current subsequence of length m=30-50 |
| **Output** | Distance to nearest historical motif, motif ID |
| **Compute** | O(n log n) for match; precomputation O(n²) offline |
| **Library** | `stumpy` |
| **Params** | `m=30-50` (subsequence length), `normalize=True` |

**How to use:**
1. **Offline:** Compute Matrix Profile on historical training data. Extract top-K motifs (recurring patterns). Label each: which ones preceded breakouts vs. which were followed by nothing.
2. **Runtime:** Use `stumpy.match(current_window, historical_data)` to find the nearest match.
3. Low distance to a "pre-breakout motif" = "current price action closely resembles a historical setup"
4. Also detect discords (current window has no match = anomalous behavior)

**Signal output:**
- `mp_distance`: distance to nearest historical motif (lower = more similar)
- `mp_motif_id`: which historical motif it matches
- `mp_motif_outcome`: what happened after the historical motif (breakout? nothing? crash?)
- `mp_is_discord`: boolean, is this window unusually unlike anything historical?

**Why:** Data-driven pattern discovery. Instead of hand-coding VCP/flag/triangle detectors, let the data tell you what recurring patterns exist and which ones predict breakouts.

---

### 1.9 LIGO-Style Template Matching — Known Consolidation Shapes

| Field | Value |
|-------|-------|
| **Input** | Trailing W=50 bars of z-normalized close prices, template bank |
| **Output** | Match score per template, best template ID |
| **Compute** | O(W × K) where K = number of templates; fast via FFT cross-correlation |
| **Library** | `scipy.signal.correlate` + custom template generation |
| **Params** | K=10-20 templates |

**How to use:**
1. **Build template bank** from historical data:
   - Extract the 30-50 bars BEFORE each confirmed breakout in training data
   - Cluster these pre-breakout windows (via DTW or Euclidean on z-normalized)
   - Centroids of top 10-20 clusters = template bank
   - Alternatively, use parametric templates: VCP(amplitude, decay, freq), ascending_triangle(slope), bull_flag(pullback_depth), flat_base(range)
2. **Runtime:** Z-normalize current 50-bar window. Cross-correlate with each template. Max correlation = match score.
3. High match score = "current price action matches a known pre-breakout consolidation shape"

**Signal output:**
- `template_score`: max correlation across all templates
- `template_id`: which template matched best
- `template_name`: human-readable (e.g., "VCP_tight", "ascending_triangle", "bull_flag_shallow")

**Why:** LIGO matched filtering detects gravitational wave signals at SNR << 1 (signal buried far below noise). Our situation on 5m candles is analogous. Template matching is the optimal linear filter for detecting known signal shapes in noise.

---

### 1.10 Wavelet Denoising — Preprocessing

| Field | Value |
|-------|-------|
| **Input** | Raw OHLCV bars |
| **Output** | Denoised OHLCV bars |
| **Compute** | O(n) |
| **Library** | `pywt` (PyWavelets) |
| **Params** | `wavelet='sym8', level=3, mode='soft'` (Stationary Wavelet Transform) |

**How to use:**
1. Apply SWT (NOT DWT — SWT is shift-invariant and has no boundary artifacts)
2. Threshold detail coefficients at each level using universal threshold (σ × √(2 ln n))
3. Reconstruct from thresholded coefficients
4. The result is a smoother price series with microstructure noise removed but structure preserved

**Signal output:**
- `denoised_close`: cleaned close price
- `denoised_high`, `denoised_low`: cleaned extremes
- `noise_energy`: energy in the removed detail coefficients (high = noisy market)

**CRITICAL:** This is a preprocessing step. Run it FIRST. Feed denoised bars to all other Layer 1 functions. Everything works better on cleaner data.

**Why:** 5m crypto candles contain significant microstructure noise (spread bouncing, market maker activity, exchange latency). Wavelet denoising removes this without introducing lag (unlike moving averages).

---

### Composite Scanner Score

All 10 Layer 1 functions produce scores. Combine into a single composite:

```python
composite = (
    w1 * pe_declining_signal          # PE declining for 5+ windows
  + w2 * bocpd_quiet_regime_signal    # recently entered quiet regime
  + w3 * kf_flat_signal              # Kalman velocity ≈ 0 (consolidating)
  + w4 * vmd_compression_score        # VMD inverse STA/LTA (coiling)
  + w5 * nplm_anomaly_percentile      # background anomaly (consolidation IS the anomaly)
  + w6 * wst_prebreakout_prob         # scattering classifier P(pre-breakout)
  + w7 * vpin_elevated                # informed flow (if tick data available)
  + w8 * (1 - mp_distance_normalized) # motif similarity (inverted: low distance = high score)
  + w9 * template_match_score         # LIGO template correlation
)
```

**Weight learning:** Logistic regression or lightweight MLP on historical labeled windows. Target: "did a breakout happen in the next 10-20 bars?"

**Threshold:** If `composite > T` → wake the LLM agent. T is tuned to balance recall vs. agent call frequency. Start at T = 0.5 (roughly top 10-20% of windows).

---

## Layer 2 — Agent Investigation

Called only when the composite scanner score exceeds the threshold. The LLM agent receives the full context packet and makes a go/no-go decision.

### Context Packet (input to agent)

```json
{
  "ticker": "SOLUSDT",
  "timeframe": "5m",
  "composite_score": 0.73,
  "scanner_signals": {
    "pe_value": 0.79, "pe_declining": true, "pe_delta": -0.008,
    "cp_prob": 0.12, "run_length": 47,
    "kf_velocity": 0.0003, "kf_acceleration": 0.00001,
    "vmd_compression": 0.68, "vmd_breakout": false,
    "nplm_score": 2.4, "nplm_percentile": 0.94,
    "wst_prebreakout_prob": 0.71,
    "vpin_value": 0.42, "vpin_elevated": true,
    "mp_distance": 0.31, "mp_motif_outcome": "breakout_up_3.2pct",
    "template_score": 0.82, "template_name": "VCP_tight"
  },
  "regime_context": {
    "hurst": 0.58,
    "hmm_state": "trending",
    "sector": "L1",
    "te_leader": false, "te_inflow": 0.034
  },
  "recent_bars": [...],  // last 100 OHLCV bars
  "volume_profile": [...]
}
```

### 2.1 Conformal Prediction Bounds — Uncertainty Quantification

| Field | Value |
|-------|-------|
| **Input** | Historical prediction errors from the signal combo that triggered |
| **Output** | Prediction interval [low, high] at desired coverage |
| **Compute** | O(n) calibration, O(1) per prediction |
| **Library** | `mapie` or manual (~20 lines) |
| **Params** | `coverage=0.90, calibration_window=200` |

**How to use:**
1. Maintain a calibration set: for each historical scanner firing with composite > T, record what actually happened (actual return over next 20 bars)
2. Compute nonconformity scores = |predicted - actual|
3. For current candidate: prediction interval = point_forecast ± quantile(nonconformity_scores, 1 - alpha)
4. **Go/no-go:** If the interval's lower bound doesn't cover transaction costs, skip
5. **Position sizing:** Size inversely proportional to interval width (wide = uncertain = small)

**Signal output:**
- `cp_lower`: lower bound of predicted return
- `cp_upper`: upper bound
- `cp_width`: interval width (uncertainty measure)

**Why:** Distribution-free coverage guarantee. "90% of the time, the actual outcome will fall within these bounds." Provides a principled basis for position sizing that doesn't rely on Gaussian assumptions (which crypto violates).

---

### 2.2 Renyi Transfer Entropy — Cross-Asset Confirmation

| Field | Value |
|-------|-------|
| **Input** | Returns of candidate + BTC + top 5 correlated assets, trailing 100 bars |
| **Output** | TE from each source to candidate; decorrelation/recorrelation signal |
| **Compute** | O(n²), acceptable because only called on ~5-20 candidates per cycle |
| **Library** | `PyCausality` or `dit` |
| **Params** | `alpha=2` (standard), `alpha=5` (tail-focused) |

**How to use:**
- Compute TE(BTC → candidate) at α=2 and α=5
- Compare current TE to trailing average TE
- **Decorrelation signal:** TE dropped significantly below average → "asset has decoupled from market" → classic pre-breakout: base-building while ignoring market noise
- **Recorrelation signal:** TE rising back toward average → "about to re-couple" → breakout imminent
- **Tail TE (α=5):** Are BTC tail events causing candidate tail events? If yes, the breakout may be BTC-driven (lower alpha). If no (independent), it's asset-specific (higher alpha).

**Signal output:**
- `rte_btc_standard`: TE from BTC at α=2
- `rte_btc_tail`: TE from BTC at α=5
- `rte_decorrelated`: boolean, has TE been below average for 20+ bars
- `rte_recorrelating`: boolean, is TE rising back toward average

**Why:** In discretionary trading, "the stock stopped moving with the market" is a classic setup indicator. Renyi TE formalizes this with information-theoretic rigor, and the alpha parameter lets you separately analyze normal-regime and tail-regime coupling.

---

### 2.3 Kronos / Foundation Model Forecast — Second Opinion

| Field | Value |
|-------|-------|
| **Input** | Last 200-500 bars of OHLCV |
| **Output** | Probabilistic forecast of next 5-20 bars |
| **Compute** | ~100ms on GPU |
| **Library** | Kronos (`github.com/shiyu-coder/Kronos`) or Chronos-2 |

**How to use:**
- Feed the candidate's recent price history to Kronos
- Get back a forecast distribution for the next 5-20 bars
- **Alignment check:** Does the forecast agree with the breakout hypothesis?
  - Forecast: up with narrow confidence → strong confirmation
  - Forecast: flat → weak confirmation (but scanner signals may still be valid — model may not detect the setup)
  - Forecast: down → contradiction → reduce conviction or skip
- **Entropy of forecast:** Low entropy = model is confident. High entropy = uncertain.

**Signal output:**
- `kronos_direction`: predicted direction (up/flat/down)
- `kronos_magnitude`: predicted move size
- `kronos_confidence`: 1 - entropy(forecast_distribution)

**Why:** A model pre-trained on 12 billion K-line records from 45 exchanges has seen more patterns than any human. Use as an independent "second opinion" — if both the signal stack AND the foundation model agree, conviction is highest.

---

### Agent Decision Logic

The LLM agent receives the full context packet (scanner signals + regime context + Layer 2 outputs) and decides:

- **ENTER** — all signals aligned, high conviction. Proceed to Layer 3.
- **SKIP** — contradictory signals, poor regime fit, or conformal bounds don't cover costs.
- **WATCHLIST** — promising but not ready. Re-check in N bars (e.g., waiting for VMD breakout trigger).

---

## Layer 3 — Execution

Active only while a position is open. Manages entries, exits, and sizing.

### 3.1 Kalman Trailing Stop — Adaptive Exit

| Field | Value |
|-------|-------|
| **Input** | Streaming close prices post-entry |
| **Output** | Trailing stop level |
| **Compute** | O(1) per bar |
| **Reuses** | Same Kalman filter from 1.3 |

**How to use:**
- Stop level = `kf_price - k * ATR`, where k adapts based on `kf_velocity`
- Strong trend (high velocity) → wider stop (let it run)
- Weakening trend (velocity declining) → tighter stop (protect gains)
- Velocity goes negative → exit immediately (trend reversed)

---

### 3.2 BOCPD Exit Signal — Regime Death Detector

| Field | Value |
|-------|-------|
| **Input** | Streaming returns post-entry |
| **Output** | P(changepoint) |
| **Compute** | O(1) per bar |
| **Reuses** | Same BOCPD from 1.2 |

**How to use:**
- If P(cp) > 0.7 while in a position → the regime that produced the entry has ended
- Tighten stop aggressively or exit immediately
- Catches momentum death before price reversal (momentum dies, THEN price drops)

---

### 3.3 Conformal Position Sizer

| Field | Value |
|-------|-------|
| **Input** | Conformal interval from 2.1, account equity, risk budget |
| **Output** | Position size in units |
| **Compute** | O(1) |

**How to use:**
```
risk_amount = equity * max_risk_per_trade  # e.g., 2% of equity
stop_distance = entry_price - stop_level   # from 3.1
raw_size = risk_amount / stop_distance

# Scale by confidence (narrow interval = more confident)
confidence_scale = 1.0 / cp_width  # normalized
position_size = min(raw_size * confidence_scale, max_position_size)
```

---

### 3.4 VMD STA/LTA Entry Trigger

| Field | Value |
|-------|-------|
| **Input** | VMD decomposition from 1.4 |
| **Output** | Binary: "breakout confirmed" |
| **Compute** | Already computed in 1.4 |

**How to use:**
- Agent decided ENTER based on the coiling signal (high compression)
- But we WAIT for the actual breakout: STA/LTA on VMD swing mode crosses 2.0
- This is the precise entry timing: the volatility expansion on the cleaned signal mode
- Avoids entering too early during the consolidation (premature entry = stop hunted)

---

## Package Dependencies

```
# Signal Processing Core
PyWavelets          # wavelet denoising (SWT)
vmdpy               # Variational Mode Decomposition
filterpy            # Kalman filter
ordpy               # permutation entropy
nolds               # DFA, Hurst exponent

# Change Detection
changepoint         # BOCPD (Rust-backed, fast)
ruptures            # offline CPD (for backtesting)

# Pattern Discovery
stumpy              # Matrix Profile, motifs, anomalies
kymatio             # Wavelet Scattering Transform
scipy               # cross-correlation (template matching)

# Machine Learning
scikit-learn        # IsolationForest (NPLM), logistic regression (composite)
xgboost             # scattering classifier
hmmlearn            # HMM regime detection
mapie               # conformal prediction

# Information Theory
PyCausality         # transfer entropy, Renyi TE

# Foundation Models (Phase 4)
kronos              # financial time series foundation model

# Execution
nautilus_trader     # backtest + live engine
```

---

## Implementation Phases

### Phase 1 — "Can we detect coiling?" (1-2 weeks)

Implement and test on historical 5m crypto data:
- 1.10 Wavelet Denoising
- 1.1 Permutation Entropy
- 1.3 Kalman Filter
- 1.2 BOCPD
- 0.1 Rolling Hurst

**Test:** Label historical breakouts. Compute signals on the 50 bars BEFORE each breakout. Do pre-breakout windows have significantly different PE / BOCPD / Kalman velocity / Hurst than random windows?

### Phase 2 — "Can we score setups?" (2-3 weeks)

- 1.4 VMD + inverse STA/LTA
- 1.5 NPLM background anomaly model
- 1.6 Wavelet Scattering + XGBoost classifier
- Composite scorer (logistic regression on all Phase 1 + Phase 2 signals)

**Test:** On a held-out validation set, what is the precision/recall curve of the composite scorer? Can we find a threshold where precision > 30% at recall > 50%? (i.e., we catch half the breakouts, and a third of our alerts are real)

### Phase 3 — "Cross-asset and pattern matching" (2-3 weeks)

- 1.8 Matrix Profile motif matching
- 1.9 LIGO template matching
- 0.3 RMT correlation filtering
- 0.4 Transfer Entropy network
- 1.7 VPIN (requires tick data pipeline)

**Test:** Do additional signals improve composite precision without destroying recall? Measure incremental lift of each new signal.

### Phase 4 — "Full agent loop" (3-4 weeks)

- 2.1 Conformal prediction bounds
- 2.2 Renyi TE cross-asset check
- 2.3 Kronos foundation model
- LLM agent prompt engineering + tool definitions
- 3.1-3.4 Execution layer
- Full backtest via NautilusTrader

**Test:** Paper trading. Compare agent decisions to backtest expectations. Monitor: hit rate, average win/loss, Sharpe, max drawdown.

---

## Data Flow Diagram

```
RAW OHLCV (100 tickers × 1 bar every 5 min)
    │
    ▼
[1.10 Wavelet Denoise] ───────────────────────────── CLEAN OHLCV
    │
    ├──→ [1.1 Permutation Entropy] ─────────→ PE score, PE_declining
    ├──→ [1.2 BOCPD] ──────────────────────→ P(changepoint), run_length
    ├──→ [1.3 Kalman Filter] ──────────────→ smoothed price, velocity, acceleration
    ├──→ [1.4 VMD + inv STA/LTA] ─────────→ compression score, breakout trigger  ⏱ every 10 bars
    ├──→ [1.5 NPLM anomaly] ──────────────→ anomaly score, percentile
    ├──→ [1.6 Scattering + XGB] ──────────→ P(pre-breakout)
    ├──→ [1.7 VPIN] ──────────────────────→ informed trading score              🔌 needs tick data
    ├──→ [1.8 Matrix Profile match] ──────→ motif distance, motif outcome       ⏱ every 10 bars
    └──→ [1.9 Template match] ────────────→ template score, template name       ⏱ every 10 bars
    │
    ▼
[COMPOSITE SCORER] ─── composite ∈ [0, 1]
    │
    │ if composite > threshold
    ▼
[LAYER 0 CONTEXT]
    ├── 0.1 Hurst regime (H value)
    ├── 0.2 HMM state (trending / choppy / quiet)
    ├── 0.3 RMT sector factors
    └── 0.4 TE network position (leader / follower / independent)
    │
    ▼
[LLM AGENT]
    ├── 2.1 Conformal prediction bounds → expected move ± uncertainty
    ├── 2.2 Renyi TE check → cross-asset confirmation
    └── 2.3 Kronos forecast → foundation model second opinion
    │
    │ ENTER / SKIP / WATCHLIST
    ▼
[EXECUTION via NautilusTrader]
    ├── 3.3 Conformal position sizer → units
    ├── 3.4 VMD STA/LTA entry trigger → precise timing
    ├── 3.1 Kalman trailing stop → adaptive exit level
    └── 3.2 BOCPD exit signal → regime death detection
```
