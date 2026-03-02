use serde::{Deserialize, Serialize};

use crate::analysis_config::AnalysisConfig;
use crate::cmaes_config::CmaEsConfig;
use crate::data_config::DataConfig;
use crate::execution_config::ExecutionConfig;
use crate::fitness_config::FitnessConfig;
use crate::ga_config::GaConfig;
use crate::matrix::{WideMask, WideMatrix};
use crate::strategy_config::StrategyConfig;

/// Tunable parameters for a single backtest run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    pub setup: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub init_cash: f64,
    pub rs_pct: f32,
    pub vol_ratio: f32,
    pub max_range_pct: f32,
    pub max_dist_52w: f32,
    pub min_adv: f32,
    pub slippage_k: f32,
    pub min_prior_move: f32,
    pub max_sma_ext: f32,
    pub regime: bool,
    pub risk_pct: f32,
    pub max_pos_pct: f32,
    /// Fraction of capital allocated to the quick half (0.0–1.0). Runner gets the rest.
    pub split_frac: f32,
    /// Minimum ADR% (ATR_14 / close). Filters out low-volatility stocks.
    pub min_adr_pct: f32,
    /// Minimum consecutive days the VCP base range stayed within max_range_pct.
    pub min_consol_days: u32,
    /// 5m bars per day (288 for crypto 5m, 1 = daily only)
    pub bars_per_day: u32,
    /// Minimum price filter (5.0 for stocks, 0.0 for crypto)
    pub min_price: f32,
    /// Minimum volume SMA filter (300_000 for stocks, lower for crypto)
    pub min_vol: f32,
    /// Signal processing parameters (None = not using signal-based strategies).
    pub signal_params: Option<crate::signal_params::SignalParams>,
    /// Fuzzy pattern recognition parameters (None = use defaults).
    pub pattern_params: Option<crate::pattern::PatternParams>,
    /// Execution constants (position sizing, slippage, hold limits).
    pub execution: ExecutionConfig,
    /// Strategy constants (signal_breakout thresholds).
    pub strategy: StrategyConfig,
    /// Fitness evaluation constants.
    pub fitness: FitnessConfig,
    /// Genetic algorithm constants.
    pub ga: GaConfig,
    /// CMA-ES optimizer constants.
    pub cmaes: CmaEsConfig,
    /// Analysis and reporting constants.
    pub analysis: AnalysisConfig,
    /// Data source configuration (benchmark symbols, parquet format, trading hours).
    pub data: DataConfig,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            setup: "breakout".into(),
            start: None,
            end: None,
            init_cash: 100_000.0,
            rs_pct: 0.02,
            vol_ratio: 1.5,
            max_range_pct: 0.15,
            max_dist_52w: 0.25,
            min_adv: 150_000_000.0,
            slippage_k: 0.1,
            min_prior_move: 0.30,
            max_sma_ext: 0.10,
            regime: true,
            risk_pct: 0.005,
            max_pos_pct: 0.20,
            split_frac: 0.50,
            min_adr_pct: 0.03,
            min_consol_days: 5,
            bars_per_day: 1,
            min_price: 5.0,
            min_vol: 300_000.0,
            signal_params: None,
            pattern_params: None,
            execution: ExecutionConfig::default(),
            strategy: StrategyConfig::default(),
            fitness: FitnessConfig::default(),
            ga: GaConfig::default(),
            cmaes: CmaEsConfig::default(),
            analysis: AnalysisConfig::default(),
            data: DataConfig::default(),
        }
    }
}

impl Params {
    /// Crypto preset: relaxed price/volume filters, BTCUSDT benchmark, 24/7 trading hours.
    /// Apply before any per-run overrides so JSON config can still narrow individual fields.
    pub fn crypto_defaults() -> Self {
        Self {
            min_price: 0.0,
            min_vol: 1_000.0,
            min_adv: 0.0,
            regime: false,
            data: DataConfig::crypto(),
            ..Default::default()
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
#[derive(Clone)]
pub struct SignalSet {
    pub entries: WideMask,
    pub exits: WideMask,
    pub stop_prices: WideMatrix,
}
