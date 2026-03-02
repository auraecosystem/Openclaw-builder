use std::fs;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "algotrader-engine",
    about = "Fast Rust backtest engine for parameter optimization"
)]
pub struct Cli {
    /// Path to the data directory containing ohlcv.parquet, cache/, etf_tickers.txt
    #[arg(long)]
    pub data_dir: String,

    /// Strategy setup: breakout, ep, parabolic
    #[arg(long, default_value = "breakout")]
    pub setup: String,

    /// Batch mode: path to JSON array of param objects
    #[arg(long)]
    pub batch: Option<String>,

    /// Serve mode: start TCP server, keep data hot, accept batch requests
    #[arg(long)]
    pub serve: bool,

    /// Port for serve mode (default: 9999)
    #[arg(long, default_value = "9999")]
    pub port: u16,

    /// Start date (YYYY-MM-DD)
    #[arg(long)]
    pub start: Option<String>,

    /// End date (YYYY-MM-DD)
    #[arg(long)]
    pub end: Option<String>,

    /// Initial cash
    #[arg(long, default_value = "100000")]
    pub init_cash: f64,

    /// RS percentile filter (top N%)
    #[arg(long, default_value = "0.02")]
    pub rs_pct: f32,

    /// Volume spike ratio threshold
    #[arg(long, default_value = "1.5")]
    pub vol_ratio: f32,

    /// Max VCP base range percentage
    #[arg(long, default_value = "0.15")]
    pub max_range: f32,

    /// Max distance from 52-week high
    #[arg(long, default_value = "0.25")]
    pub max_dist_52w: f32,

    /// Minimum average dollar volume
    #[arg(long, default_value = "150000000")]
    pub min_adv: f32,

    /// Slippage impact factor
    #[arg(long, default_value = "0.1")]
    pub slippage_k: f32,

    /// Minimum 3-month prior move
    #[arg(long, default_value = "0.30")]
    pub min_prior_move: f32,

    /// Max SMA extension percentage
    #[arg(long, default_value = "0.10")]
    pub max_sma_ext: f32,

    /// Disable SPY regime filter
    #[arg(long)]
    pub no_regime: bool,

    /// Risk percentage per trade
    #[arg(long, default_value = "0.005")]
    pub risk_pct: f32,

    /// Max position size as fraction of portfolio
    #[arg(long, default_value = "0.20")]
    pub max_pos_pct: f32,

    /// Quick half's share of capital (0.0-1.0). Runner gets the rest.
    #[arg(long, default_value = "0.50")]
    pub split_frac: f32,

    /// Minimum ADR% (ATR_14 / close)
    #[arg(long, default_value = "0.03")]
    pub min_adr_pct: f32,

    /// Minimum consecutive days in consolidation
    #[arg(long, default_value = "5")]
    pub min_consol_days: u32,

    /// 5m bars per day (288 for crypto, 1 = daily only)
    #[arg(long, default_value = "1")]
    pub bars_per_day: u32,

    /// Minimum price filter (5.0 for stocks, 0.0 for crypto)
    #[arg(long, default_value = "5.0")]
    pub min_price: f32,

    /// Minimum volume SMA filter
    #[arg(long, default_value = "300000")]
    pub min_vol: f32,

    // -- Evolution options --------------------------------------------------

    /// Run evolutionary optimization
    #[arg(long)]
    pub evolve: bool,

    /// Evolution algorithm: cmaes, ga
    #[arg(long, default_value = "cmaes")]
    pub algo: String,

    /// Number of evolution generations
    #[arg(long, default_value = "50")]
    pub generations: usize,

    /// Population size for evolution (0 = algorithm default)
    #[arg(long, default_value = "0")]
    pub pop_size_evo: usize,

    /// Fitness metric: sharpe, sortino, return, profit_factor, cagr, growth, composite
    #[arg(long, default_value = "composite")]
    pub fitness: String,

    /// CMA-ES initial step size
    #[arg(long, default_value = "0.3")]
    pub sigma: f64,

    /// Also evolve SignalParams (44D instead of 14D)
    #[arg(long)]
    pub evolve_signals: bool,

    /// Also evolve PatternParams (bull flag tuning, +19D)
    #[arg(long)]
    pub evolve_patterns: bool,

    /// Use crypto parameter ranges
    #[arg(long)]
    pub crypto: bool,

    /// Walk-forward folds (0 = no walk-forward)
    #[arg(long, default_value = "0")]
    pub wf_folds: usize,

    /// RNG seed for reproducibility
    #[arg(long)]
    pub seed: Option<u64>,

    /// Dump individual trades to CSV for cross-engine validation
    #[arg(long)]
    pub dump_trades: Option<String>,

    /// Load base params from a JSON config file (CLI flags override)
    #[arg(long)]
    pub config: Option<String>,
}

impl Cli {
    /// Build Params from CLI args, optionally loading a JSON config as base.
    ///
    /// When `--config path.json` is provided, the JSON is deserialized as the
    /// base Params (preserving signal_params, pattern_params, fitness, etc.).
    /// CLI flags then override the core fields on top of that base.
    pub fn to_params(&self) -> algotrader_engine::types::Params {
        let mut p = if let Some(ref cfg_path) = self.config {
            let content = fs::read_to_string(cfg_path)
                .unwrap_or_else(|e| panic!("Failed to read config {cfg_path}: {e}"));
            serde_json::from_str::<algotrader_engine::types::Params>(&content)
                .unwrap_or_else(|e| panic!("Failed to parse config {cfg_path}: {e}"))
        } else {
            algotrader_engine::types::Params::default()
        };

        // CLI flags always override core fields.
        p.setup = self.setup.clone();
        p.start = self.start.clone();
        p.end = self.end.clone();
        p.init_cash = self.init_cash;
        p.rs_pct = self.rs_pct;
        p.vol_ratio = self.vol_ratio;
        p.max_range_pct = self.max_range;
        p.max_dist_52w = self.max_dist_52w;
        p.min_adv = self.min_adv;
        p.slippage_k = self.slippage_k;
        p.min_prior_move = self.min_prior_move;
        p.max_sma_ext = self.max_sma_ext;
        p.regime = !self.no_regime;
        p.risk_pct = self.risk_pct;
        p.max_pos_pct = self.max_pos_pct;
        p.split_frac = self.split_frac;
        p.min_adr_pct = self.min_adr_pct;
        p.min_consol_days = self.min_consol_days;
        p.bars_per_day = self.bars_per_day;
        p.min_price = self.min_price;
        p.min_vol = self.min_vol;

        p
    }
}
