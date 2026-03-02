//! Fuzzy pattern recognition types.
//!
//! Patterns are scored as continuous confidence values in [0, 1] rather than
//! binary present/absent flags. Each pattern decomposes into component scores
//! (pole strength, slope, tightness, volume profile) combined via weighted mean.
//!
//! Multiple timeframes are scored independently; the final composite uses
//! `pattern_scorer_weights` to blend across timeframes.

use serde::{Deserialize, Serialize};

/// Number of pattern confidence features per ticker (one per pattern kind ×
/// timeframe). Currently: 1 pattern × 4 timeframes = 4.
pub const PATTERN_FEATURE_COUNT: usize = 4;

/// Timeframe index order: [daily, 1h, 30m, 5m].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternTimeframe {
    Daily = 0,
    H1 = 1,
    M30 = 2,
    M5 = 3,
}

/// Available pattern kinds. Extend as new patterns are added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternKind {
    BullFlag,
}

/// Fixed-size array holding pattern confidence scores for one ticker.
///
/// Layout: `[daily_bull_flag, 1h_bull_flag, 30m_bull_flag, 5m_bull_flag]`.
/// All values are in [0.0, 1.0]; NaN = insufficient data.
pub type PatternScores = [f32; PATTERN_FEATURE_COUNT];

/// Tunable knobs for fuzzy pattern recognition.
///
/// All serde defaults produce sensible starting values calibrated on
/// Qullamaggie-style crypto momentum data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternParams {
    // -- Bull flag: pole component ------------------------------------------

    /// Sigmoid midpoint for pole return (5% minimum strong move).
    #[serde(default = "default_flag_pole_mid")]
    pub flag_pole_mid: f32,

    /// Sigmoid steepness for pole return (higher = sharper threshold).
    #[serde(default = "default_flag_pole_k")]
    pub flag_pole_k: f32,

    /// Minimum bars required for the pole (too few = noise, not a real move).
    #[serde(default = "default_flag_pole_min_bars")]
    pub flag_pole_min_bars: usize,

    // -- Bull flag: slope component -----------------------------------------

    /// Gaussian center for flag slope (slight downward drift is ideal, ≈ -0.1 ATR/bar).
    #[serde(default = "default_flag_slope_center")]
    pub flag_slope_center: f32,

    /// Gaussian width for flag slope tolerance.
    #[serde(default = "default_flag_slope_sigma")]
    pub flag_slope_sigma: f32,

    // -- Bull flag: tightness (retracement) component -----------------------

    /// Retrace below this → full score (flag is tight, shallow pullback).
    #[serde(default = "default_flag_retrace_lo")]
    pub flag_retrace_lo: f32,

    /// Retrace above this → zero score (flag is too deep, absorbed the pole).
    #[serde(default = "default_flag_retrace_hi")]
    pub flag_retrace_hi: f32,

    // -- Bull flag: volume component ----------------------------------------

    /// Sigmoid midpoint for volume ratio (flag vol / pole vol).
    /// Score is high when flag volume is lower (< 0.7 of pole).
    #[serde(default = "default_flag_vol_mid")]
    pub flag_vol_mid: f32,

    /// Sigmoid steepness for volume ratio (negative = lower ratio → higher score).
    #[serde(default = "default_flag_vol_k")]
    pub flag_vol_k: f32,

    // -- Bull flag: geometry ------------------------------------------------

    /// Maximum bars for the flag consolidation phase.
    #[serde(default = "default_flag_max_bars")]
    pub flag_max_bars: usize,

    /// Total lookback for pole search (bars before current bar to scan).
    #[serde(default = "default_flag_lookback")]
    pub flag_lookback: usize,

    // -- Bull flag: component weights ---------------------------------------

    /// Weights for [pole, slope, retracement, volume] components.
    /// Must sum to 1.0 (not enforced; scorer normalizes internally).
    #[serde(default = "default_flag_weights")]
    pub flag_weights: [f32; 4],

    // -- Multi-timeframe composite ------------------------------------------

    /// Per-timeframe weights for [daily, 1h, 30m, 5m] pattern scores.
    #[serde(default = "default_pattern_scorer_weights")]
    pub pattern_scorer_weights: [f32; PATTERN_FEATURE_COUNT],

    /// Additive bias applied to the weighted pattern sum before sigmoid.
    #[serde(default = "default_pattern_scorer_bias")]
    pub pattern_scorer_bias: f32,

    /// Blend factor: `alpha * signal_score + (1 - alpha) * pattern_score`.
    /// 0.0 = pure pattern, 1.0 = pure signal pipeline.
    #[serde(default = "default_pattern_alpha")]
    pub pattern_alpha: f32,
}

// ---------------------------------------------------------------------------
// Serde defaults
// ---------------------------------------------------------------------------

fn default_flag_pole_mid() -> f32 { 0.05 }
fn default_flag_pole_k() -> f32 { 20.0 }
fn default_flag_pole_min_bars() -> usize { 5 }

fn default_flag_slope_center() -> f32 { -0.1 }
fn default_flag_slope_sigma() -> f32 { 0.3 }

fn default_flag_retrace_lo() -> f32 { 0.33 }
fn default_flag_retrace_hi() -> f32 { 0.50 }

fn default_flag_vol_mid() -> f32 { 0.7 }
fn default_flag_vol_k() -> f32 { -10.0 }

fn default_flag_max_bars() -> usize { 30 }
fn default_flag_lookback() -> usize { 60 }

fn default_flag_weights() -> [f32; 4] { [0.30, 0.20, 0.30, 0.20] }

fn default_pattern_scorer_weights() -> [f32; PATTERN_FEATURE_COUNT] {
    [1.0 / PATTERN_FEATURE_COUNT as f32; PATTERN_FEATURE_COUNT]
}
fn default_pattern_scorer_bias() -> f32 { 0.0 }
fn default_pattern_alpha() -> f32 { 0.5 }

// ---------------------------------------------------------------------------
// Default impl
// ---------------------------------------------------------------------------

impl Default for PatternParams {
    fn default() -> Self {
        Self {
            flag_pole_mid: default_flag_pole_mid(),
            flag_pole_k: default_flag_pole_k(),
            flag_pole_min_bars: default_flag_pole_min_bars(),
            flag_slope_center: default_flag_slope_center(),
            flag_slope_sigma: default_flag_slope_sigma(),
            flag_retrace_lo: default_flag_retrace_lo(),
            flag_retrace_hi: default_flag_retrace_hi(),
            flag_vol_mid: default_flag_vol_mid(),
            flag_vol_k: default_flag_vol_k(),
            flag_max_bars: default_flag_max_bars(),
            flag_lookback: default_flag_lookback(),
            flag_weights: default_flag_weights(),
            pattern_scorer_weights: default_pattern_scorer_weights(),
            pattern_scorer_bias: default_pattern_scorer_bias(),
            pattern_alpha: default_pattern_alpha(),
        }
    }
}
