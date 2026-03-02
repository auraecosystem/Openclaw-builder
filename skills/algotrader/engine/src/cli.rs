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
}

impl Cli {
    pub fn to_params(&self) -> algotrader_engine::types::Params {
        algotrader_engine::types::Params {
            setup: self.setup.clone(),
            start: self.start.clone(),
            end: self.end.clone(),
            init_cash: self.init_cash,
            rs_pct: self.rs_pct,
            vol_ratio: self.vol_ratio,
            max_range_pct: self.max_range,
            max_dist_52w: self.max_dist_52w,
            min_adv: self.min_adv,
            slippage_k: self.slippage_k,
            min_prior_move: self.min_prior_move,
            max_sma_ext: self.max_sma_ext,
            regime: !self.no_regime,
            risk_pct: self.risk_pct,
            max_pos_pct: self.max_pos_pct,
            split_frac: self.split_frac,
            min_adr_pct: self.min_adr_pct,
            min_consol_days: self.min_consol_days,
            bars_per_day: self.bars_per_day,
            min_price: self.min_price,
            min_vol: self.min_vol,
            signal_params: None,
            execution: Default::default(),
            strategy: Default::default(),
            fitness: Default::default(),
            ga: Default::default(),
            cmaes: Default::default(),
            analysis: Default::default(),
        }
    }
}
