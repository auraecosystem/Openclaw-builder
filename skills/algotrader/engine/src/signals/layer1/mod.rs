//! Layer 1 -- Scanner.
//!
//! Runs every bar across all tickers.  Each of the 13 sub-algorithms
//! produces a single normalized f32 score collected into `ScannerOutput`.
//! The composite scorer then maps these to a candidate probability via a
//! weighted dot product + sigmoid.

pub mod bocpd;
pub mod kalman;
pub mod knn_anomaly;
pub mod perm_entropy;
pub mod scattering;
pub mod scorer;
pub mod stomp;
pub mod swt;
pub mod template;
pub mod vmd;
pub mod vpin;

pub use crate::signals::feature::SCANNER_FEATURE_COUNT;

/// Aggregated output of all Layer 1 scanner algorithms for one bar of one
/// ticker.
///
/// Fields store **raw** algorithm outputs so Layer 3 can consume them at
/// their natural scale (e.g. `kalman_level` in price units for the trailing
/// stop).  The scorer receives normalized features via `to_scorer_array()`.
#[derive(Clone, Debug, Default)]
pub struct ScannerOutput {
    /// 1.1  Permutation entropy (0 = perfectly ordered, 1 = random).
    pub perm_entropy: f32,
    /// 1.2  BOCPD changepoint probability at current bar.
    pub bocpd_prob: f32,
    /// 1.3  Kalman-filtered price level (raw, in price units).
    pub kalman_level: f32,
    /// 1.3  Kalman-filtered trend velocity (raw, price units/bar).
    pub kalman_trend: f32,
    /// 1.4  VMD STA/LTA ratio on the swing mode (raw, typically 0–10).
    pub vmd_sta_lta: f32,
    /// 1.5  KNN anomaly distance score.
    pub knn_anomaly: f32,
    /// 1.6  Wavelet scattering classification score.
    pub scatter_class: f32,
    /// 1.7  VPIN informed-trading estimate.
    pub vpin: f32,
    /// 1.8  Matrix profile novelty (inverted nearest-motif distance).
    pub mp_novelty: f32,
    /// 1.9  Template matching max cross-correlation.
    pub template_corr: f32,
    /// 1.10 SWT denoised price output (raw, in price units).
    pub swt_denoised: f32,
    /// 0.1  Hurst exponent pass-through from Layer 0.
    pub hurst: f32,
    /// 0.2  HMM state pass-through from Layer 0 (raw integer cast to f32).
    pub hmm_state: f32,
    /// Last close price — used by `to_scorer_array()` for normalization.
    pub last_close: f32,
}

impl ScannerOutput {
    /// Normalize features to ~[0,1] or ~[-1,1] for the composite scorer.
    ///
    /// Price-scale features (kalman, swt) are divided by `last_close` so the
    /// dot product isn't dominated by absolute price level.  Raw fields are
    /// preserved for Layer 3 (trailing stop, BOCPD exit, VMD entry trigger).
    pub fn to_scorer_array(&self) -> [f32; SCANNER_FEATURE_COUNT] {
        let lc = self.last_close;

        // Kalman trend as fractional velocity (% of price per bar)
        let norm_kalman_vel = if lc.abs() > 1e-10 {
            (self.kalman_trend / lc).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        // SWT deviation from close as fraction of price
        let norm_swt = if lc.abs() > 1e-10 {
            ((self.swt_denoised - lc) / lc).clamp(-0.1, 0.1) * 10.0
        } else {
            0.0
        };

        // VMD STA/LTA capped to [0, 1]
        let norm_vmd = (self.vmd_sta_lta / 5.0).clamp(0.0, 1.0);

        // HMM state: {0,1,2} → {0, 0.5, 1}
        let norm_hmm = self.hmm_state / 2.0;

        [
            self.perm_entropy,
            self.bocpd_prob,
            norm_kalman_vel,   // kalman_level slot: fractional velocity
            norm_kalman_vel,   // kalman_trend slot: fractional velocity
            norm_vmd,
            self.knn_anomaly,
            self.scatter_class,
            self.vpin,
            self.mp_novelty,
            self.template_corr,
            norm_swt,
            self.hurst,
            norm_hmm,
        ]
    }
}

/// Run all Layer 1 scanner algorithms for one bar on one ticker.
///
/// Calls each of the 10 sub-algorithms (+ 2 pass-throughs from L0) and
/// collects the 13 features into `ScannerOutput`.  Stateful algorithms
/// (Kalman, BOCPD) are updated in-place via the arena.
///
/// KNN anomaly (1.5), scattering (1.6), and template matching (1.9) return
/// neutral defaults until reference data / trained weights are available.
pub fn run_scanner(
    close_window: &[f32],
    volume_window: &[f32],
    arena: &mut crate::signals::arena::ThreadArena,
    params: &crate::signals::params::SignalParams,
    hurst: f32,
    hmm_state: u8,
) -> ScannerOutput {
    let w = close_window.len();
    if w < 2 {
        return ScannerOutput::default();
    }
    let last_close = close_window[w - 1];

    // 1.1 Permutation entropy
    let pe = perm_entropy::compute_perm_entropy(
        close_window,
        params.pe_order,
        params.pe_delay,
    );

    // 1.2 BOCPD (stateful — fed the latest close-to-close return)
    let ret = if w >= 2 {
        close_window[w - 1] - close_window[w - 2]
    } else {
        0.0
    };
    let bocpd_prob = arena.bocpd_state.update(ret, params.bocpd_lambda);

    // 1.3 Kalman filter (stateful — [level, trend])
    let (k_level, k_trend) = kalman::kalman_update(
        &mut arena.kalman_x,
        &mut arena.kalman_p,
        last_close,
        params.kalman_q,
        params.kalman_r,
    );

    // 1.4 VMD STA/LTA
    let vmd = vmd::compute_vmd_sta_lta(
        close_window,
        params.vmd_k_modes,
        params.vmd_alpha,
        params.vmd_sta_window,
        params.vmd_lta_window,
        &mut arena.fft_scratch,
    );

    // 1.5 KNN anomaly (needs historical feature matrix — neutral for now)
    let knn = 0.0_f32;

    // 1.6 Scattering (needs trained classifier weights — neutral for now)
    let scatter = 0.5_f32;

    // 1.7 VPIN
    let vpin_score = if !volume_window.is_empty() && volume_window.len() == w {
        vpin::compute_vpin(close_window, volume_window, params.vpin_n_buckets)
    } else {
        0.0
    };

    // 1.8 STOMP matrix profile novelty
    let m = params.stomp_m.min(w);
    let mp_dist = if w > m && m >= 2 {
        stomp::compute_stomp(
            close_window,
            &close_window[w - m..],
            m,
            &mut arena.fft_scratch,
        )
    } else {
        f32::MAX
    };
    // Invert distance to novelty score in [0, 1].
    let mp_novelty = if (0.0..f32::MAX).contains(&mp_dist) {
        1.0 / (1.0 + mp_dist)
    } else {
        0.0
    };

    // 1.9 Template matching (needs template bank — neutral for now)
    let template = 0.0_f32;

    // 1.10 SWT denoise
    let swt_val = swt::compute_swt_denoise(
        close_window,
        params.swt_level,
        &mut arena.fft_scratch,
    );

    // Store raw values — normalization happens in to_scorer_array().
    ScannerOutput {
        perm_entropy: pe,
        bocpd_prob,
        kalman_level: k_level,
        kalman_trend: k_trend,
        vmd_sta_lta: vmd,
        knn_anomaly: knn,
        scatter_class: scatter,
        vpin: vpin_score,
        mp_novelty,
        template_corr: template,
        swt_denoised: swt_val,
        hurst,
        hmm_state: hmm_state as f32,
        last_close,
    }
}
