use serde::{Deserialize, Serialize};

use crate::analysis_config::AnalysisConfig;
use crate::cmaes_config::CmaEsConfig;
use crate::execution_config::ExecutionConfig;
use crate::fitness_config::FitnessConfig;
use crate::ga_config::GaConfig;
use crate::matrix::{WideMask, WideMatrix};
use crate::strategy_config::StrategyConfig;

/// Tunable parameters for a single backtest run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "default_setup")]
    pub setup: String,
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub end: Option<String>,
    #[serde(default = "default_init_cash")]
    pub init_cash: f64,
    #[serde(default = "default_rs_pct")]
    pub rs_pct: f32,
    #[serde(default = "default_vol_ratio")]
    pub vol_ratio: f32,
    #[serde(default = "default_max_range_pct")]
    pub max_range_pct: f32,
    #[serde(default = "default_max_dist_52w")]
    pub max_dist_52w: f32,
    #[serde(default = "default_min_adv")]
    pub min_adv: f32,
    #[serde(default = "default_slippage_k")]
    pub slippage_k: f32,
    #[serde(default = "default_min_prior_move")]
    pub min_prior_move: f32,
    #[serde(default = "default_max_sma_ext")]
    pub max_sma_ext: f32,
    #[serde(default = "default_true")]
    pub regime: bool,
    #[serde(default = "default_risk_pct")]
    pub risk_pct: f32,
    #[serde(default = "default_max_pos_pct")]
    pub max_pos_pct: f32,
    /// Fraction of capital allocated to the quick half (0.0–1.0). Runner gets the rest.
    #[serde(default = "default_split_frac")]
    pub split_frac: f32,
    /// Minimum ADR% (ATR_14 / close). Filters out low-volatility stocks.
    #[serde(default = "default_min_adr_pct")]
    pub min_adr_pct: f32,
    /// Minimum consecutive days the VCP base range stayed within max_range_pct.
    #[serde(default = "default_min_consol_days")]
    pub min_consol_days: u32,
    /// 5m bars per day (288 for crypto 5m, 1 = daily only)
    #[serde(default = "default_bars_per_day")]
    pub bars_per_day: u32,
    /// Minimum price filter (5.0 for stocks, 0.0 for crypto)
    #[serde(default = "default_min_price")]
    pub min_price: f32,
    /// Minimum volume SMA filter (300_000 for stocks, lower for crypto)
    #[serde(default = "default_min_vol")]
    pub min_vol: f32,
    /// Signal processing parameters (None = not using signal-based strategies).
    #[serde(default)]
    pub signal_params: Option<crate::signal_params::SignalParams>,

    /// Execution constants (position sizing, slippage, hold limits).
    #[serde(default)]
    pub execution: ExecutionConfig,

    /// Strategy constants (signal_breakout thresholds).
    #[serde(default)]
    pub strategy: StrategyConfig,

    /// Fitness evaluation constants.
    #[serde(default)]
    pub fitness: FitnessConfig,

    /// Genetic algorithm constants.
    #[serde(default)]
    pub ga: GaConfig,

    /// CMA-ES optimizer constants.
    #[serde(default)]
    pub cmaes: CmaEsConfig,

    /// Analysis and reporting constants.
    #[serde(default)]
    pub analysis: AnalysisConfig,
}

fn default_setup() -> String {
    "breakout".into()
}
fn default_init_cash() -> f64 {
    100_000.0
}
fn default_rs_pct() -> f32 {
    0.02
}
fn default_vol_ratio() -> f32 {
    1.5
}
fn default_max_range_pct() -> f32 {
    0.15
}
fn default_max_dist_52w() -> f32 {
    0.25
}
fn default_min_adv() -> f32 {
    150_000_000.0
}
fn default_slippage_k() -> f32 {
    0.1
}
fn default_min_prior_move() -> f32 {
    0.30
}
fn default_max_sma_ext() -> f32 {
    0.10
}
fn default_true() -> bool {
    true
}
fn default_risk_pct() -> f32 {
    0.005
}
fn default_max_pos_pct() -> f32 {
    0.20
}
fn default_split_frac() -> f32 {
    0.50
}
fn default_min_adr_pct() -> f32 {
    0.03
}
fn default_min_consol_days() -> u32 {
    5
}
fn default_bars_per_day() -> u32 {
    1
}
fn default_min_price() -> f32 {
    5.0
}
fn default_min_vol() -> f32 {
    300_000.0
}

impl Default for Params {
    fn default() -> Self {
        Self {
            setup: default_setup(),
            start: None,
            end: None,
            init_cash: default_init_cash(),
            rs_pct: default_rs_pct(),
            vol_ratio: default_vol_ratio(),
            max_range_pct: default_max_range_pct(),
            max_dist_52w: default_max_dist_52w(),
            min_adv: default_min_adv(),
            slippage_k: default_slippage_k(),
            min_prior_move: default_min_prior_move(),
            max_sma_ext: default_max_sma_ext(),
            regime: default_true(),
            risk_pct: default_risk_pct(),
            max_pos_pct: default_max_pos_pct(),
            split_frac: default_split_frac(),
            min_adr_pct: default_min_adr_pct(),
            min_consol_days: default_min_consol_days(),
            bars_per_day: default_bars_per_day(),
            min_price: default_min_price(),
            min_vol: default_min_vol(),
            signal_params: None,
            execution: ExecutionConfig::default(),
            strategy: StrategyConfig::default(),
            fitness: FitnessConfig::default(),
            ga: GaConfig::default(),
            cmaes: CmaEsConfig::default(),
            analysis: AnalysisConfig::default(),
        }
    }
}

/// Resolved date range as row indices (computed once per batch).
pub struct ResolvedParams {
    pub params: Params,
    pub start_row: usize,
    pub end_row: usize, // exclusive
}

#[derive(Clone, Debug)]
pub struct Trade {
    pub ticker_col: usize,
    pub setup: &'static str,
    pub entry_row: usize,
    pub exit_row: usize,
    pub entry_price: f32,
    pub exit_price: f32,
    pub initial_stop: f32,
    pub shares: f32,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub direction: Direction,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum Direction {
    Long,
    Short,
}

/// Entry/exit/stop signals for one setup.
pub struct SignalSet {
    pub entries: WideMask,
    pub exits: WideMask,
    pub stop_prices: WideMatrix,
}
