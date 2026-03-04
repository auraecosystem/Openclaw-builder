//! PyO3 Python bridge for the signal processing pipeline.
//!
//! Exposes `SignalEngine` — a batch-oriented class that processes all tickers
//! in parallel using rayon, returning numpy arrays of signal features.
//!
//! Build:  `cd engine && maturin develop --features python`
//! Usage:  `from algotrader_signals import SignalEngine`

use pyo3::prelude::*;
use pyo3::types::PyDict;
use rayon::prelude::*;

use engine_signals::pipeline::{self, CharacterizationState};
use engine_signals::ThreadArena;
use engine_types::{SignalParams, SCANNER_FEATURE_COUNT};

// ---------------------------------------------------------------------------
// SignalEngine: the main Python-facing class
// ---------------------------------------------------------------------------

/// Batch signal feature extractor.
///
/// Wraps the Rust signal pipeline. Feed it a (n_bars, n_tickers) returns
/// matrix and get back (n_bars, n_tickers, 13) normalized features.
///
/// ```python
/// engine = SignalEngine()                     # default params
/// engine = SignalEngine(window_len=200)       # override params
/// features = engine.run_batch(returns_2d)     # → list of list of list[float]
/// ```
#[pyclass]
struct SignalEngine {
    params: SignalParams,
}

#[pymethods]
impl SignalEngine {
    /// Create a new SignalEngine with optional parameter overrides.
    ///
    /// Accepts keyword arguments matching SignalParams fields. Unspecified
    /// fields use sensible defaults.
    #[new]
    #[pyo3(signature = (**kwargs))]
    fn new(kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut params = SignalParams::default();

        if let Some(kw) = kwargs {
            // Apply overrides from Python kwargs
            macro_rules! set_field {
                ($kw:expr, $params:expr, $name:literal, $field:ident, $t:ty) => {
                    if let Some(val) = $kw.get_item($name)? {
                        $params.$field = val.extract::<$t>()?;
                    }
                };
            }

            set_field!(kw, params, "window_len", window_len, usize);
            set_field!(kw, params, "candidate_threshold", candidate_threshold, f32);
            set_field!(kw, params, "l0_recompute_interval", l0_recompute_interval, usize);
            set_field!(kw, params, "pe_order", pe_order, usize);
            set_field!(kw, params, "pe_delay", pe_delay, usize);
            set_field!(kw, params, "pe_window", pe_window, usize);
            set_field!(kw, params, "bocpd_lambda", bocpd_lambda, f32);
            set_field!(kw, params, "kalman_q", kalman_q, f32);
            set_field!(kw, params, "kalman_r", kalman_r, f32);
            set_field!(kw, params, "vmd_k_modes", vmd_k_modes, usize);
            set_field!(kw, params, "vmd_alpha", vmd_alpha, f32);
            set_field!(kw, params, "vmd_sta_window", vmd_sta_window, usize);
            set_field!(kw, params, "vmd_lta_window", vmd_lta_window, usize);
            set_field!(kw, params, "knn_k", knn_k, usize);
            set_field!(kw, params, "stomp_m", stomp_m, usize);
            set_field!(kw, params, "swt_level", swt_level, usize);
            set_field!(kw, params, "hurst_window", hurst_window, usize);
            set_field!(kw, params, "hmm_n_states", hmm_n_states, usize);
            set_field!(kw, params, "rmt_window", rmt_window, usize);
            set_field!(kw, params, "vpin_n_buckets", vpin_n_buckets, usize);
            set_field!(kw, params, "atr_period", atr_period, usize);

            // Scorer weights: accept list of 13 floats
            if let Some(val) = kw.get_item("scorer_weights")? {
                let weights: Vec<f32> = val.extract()?;
                if weights.len() == SCANNER_FEATURE_COUNT {
                    params.scorer_weights.copy_from_slice(&weights);
                }
            }
            set_field!(kw, params, "scorer_bias", scorer_bias, f32);
        }

        Ok(Self { params })
    }

    /// Process a batch of close-price matrices for all tickers.
    ///
    /// Args:
    ///     close_matrix: list[list[float]] — (n_tickers, n_bars) close prices
    ///     volume_matrix: list[list[float]] — (n_tickers, n_bars) volumes (optional,
    ///                    pass empty lists to skip VPIN)
    ///
    /// Returns:
    ///     list[list[list[float]]] — (n_tickers, n_bars, 13) normalized features
    ///
    /// Each ticker is processed independently in parallel using rayon.
    /// Stateful algorithms (Kalman, BOCPD) reset between tickers.
    fn run_batch(
        &self,
        close_matrix: Vec<Vec<f32>>,
        volume_matrix: Vec<Vec<f32>>,
    ) -> PyResult<Vec<Vec<Vec<f32>>>> {
        let params = &self.params;
        let n_tickers = close_matrix.len();

        // Ensure volume_matrix has the right length (pad with empty if needed)
        let vol_matrix: Vec<Vec<f32>> = if volume_matrix.len() == n_tickers {
            volume_matrix
        } else {
            (0..n_tickers).map(|i| {
                volume_matrix.get(i).cloned().unwrap_or_default()
            }).collect()
        };

        // Process all tickers in parallel
        let results: Vec<Vec<Vec<f32>>> = close_matrix
            .into_par_iter()
            .zip(vol_matrix.into_par_iter())
            .map(|(close_series, vol_series)| {
                process_one_ticker(&close_series, &vol_series, params)
            })
            .collect();

        Ok(results)
    }

    /// Process a single ticker's close prices.
    ///
    /// Args:
    ///     close: list[float] — n_bars close prices
    ///     volume: list[float] — n_bars volumes (can be empty)
    ///
    /// Returns:
    ///     list[list[float]] — (n_bars, 13) normalized features
    fn run_single(
        &self,
        close: Vec<f32>,
        volume: Vec<f32>,
    ) -> PyResult<Vec<Vec<f32>>> {
        Ok(process_one_ticker(&close, &volume, &self.params))
    }

    /// Get the number of scanner features (13).
    #[staticmethod]
    fn feature_count() -> usize {
        SCANNER_FEATURE_COUNT
    }

    /// Get feature names in order (matches array columns).
    #[staticmethod]
    fn feature_names() -> Vec<String> {
        vec![
            "perm_entropy".into(),
            "bocpd_prob".into(),
            "kalman_level".into(),
            "kalman_trend".into(),
            "vmd_sta_lta".into(),
            "knn_anomaly".into(),
            "scatter_class".into(),
            "vpin".into(),
            "mp_novelty".into(),
            "template_corr".into(),
            "swt_denoised".into(),
            "hurst".into(),
            "hmm_state".into(),
        ]
    }
}

// ---------------------------------------------------------------------------
// Internal processing
// ---------------------------------------------------------------------------

/// Process one ticker: run the full signal pipeline bar-by-bar, returning
/// normalized feature vectors.
fn process_one_ticker(
    close: &[f32],
    volume: &[f32],
    params: &SignalParams,
) -> Vec<Vec<f32>> {
    let n_bars = close.len();
    let w = params.window_len;

    if n_bars < 2 {
        return vec![vec![0.0; SCANNER_FEATURE_COUNT]; n_bars];
    }

    let mut arena = ThreadArena::new(params);
    let mut l0_state = CharacterizationState::default();
    let mut features = Vec::with_capacity(n_bars);

    // Compute simple returns for characterization
    let mut returns = vec![0.0_f32; n_bars];
    for i in 1..n_bars {
        if close[i - 1].abs() > 1e-10 {
            returns[i] = (close[i] - close[i - 1]) / close[i - 1];
        }
    }

    for bar_idx in 0..n_bars {
        // Not enough history yet — emit zeros
        if bar_idx < 2 {
            features.push(vec![0.0; SCANNER_FEATURE_COUNT]);
            continue;
        }

        // Extract windows
        let start = if bar_idx >= w { bar_idx - w } else { 0 };
        let close_window = &close[start..=bar_idx];
        let vol_window = if !volume.is_empty() && volume.len() > bar_idx {
            &volume[start..=bar_idx]
        } else {
            &[] as &[f32]
        };

        // Recompute characterization when due
        let ret_window = &returns[start..=bar_idx];
        l0_state = pipeline::characterize(
            close_window,
            ret_window,
            None, // no cross-asset returns for batch mode
            bar_idx,
            params,
            &l0_state,
        );

        // Run full pipeline
        let output = pipeline::run_pipeline(
            close_window,
            vol_window,
            &[], // high (not needed for scanner algorithms)
            &[], // low  (not needed for scanner algorithms)
            &mut arena,
            params,
            &l0_state,
        );

        // Normalize and collect
        let norm = output.scanner.normalize_for_scorer(params);
        features.push(norm.to_vec());
    }

    features
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// Python module: `algotrader_engine`
///
/// ```python
/// from algotrader_engine import SignalEngine
///
/// engine = SignalEngine(window_len=200)
/// features = engine.run_batch(close_matrix, volume_matrix)
/// ```
#[pymodule]
pub fn algotrader_engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SignalEngine>()?;
    Ok(())
}
