//! Strategy module: trait-based setup abstraction for backtest strategies.
//!
//! Each setup (breakout, EP, parabolic) implements the `Setup` trait, providing
//! its own universe filter, signal generation, exit rules, and fill mode. The
//! `create_setups()` factory maps a setup name to one or more concrete Setup
//! implementations that the simulation engine runs independently.

pub use engine_types::strategy_config as config;
mod breakout;
mod ep;
mod parabolic;
mod pattern_breakout;
mod signal_breakout;

use std::ops::Range;
use std::path::PathBuf;

use engine_data::{DataStore, Indicator, WideMask};
use engine_pipeline::config::FillModeConfig;
use engine_pipeline::DynamicExitRule;
use crate::execution::{ExitRule, FillMode};
use engine_types::{Direction, Params, SignalSet, WideMatrix};

pub use breakout::{BreakoutQuick, BreakoutRunner};
pub use ep::EpisodicPivot;
pub use parabolic::ParabolicShort;
pub use pattern_breakout::PatternBreakout;
pub use signal_breakout::SignalBreakout;

/// Core abstraction: a trading setup that can filter a universe, generate
/// signals, and declare its execution parameters.
pub trait Setup: Send + Sync {
    /// Human-readable identifier used in trade logs and reports.
    fn name(&self) -> &'static str;

    /// Whether this setup trades long or short. Defaults to Long.
    fn direction(&self) -> Direction {
        Direction::Long
    }

    /// Fraction of total equity allocated to this sub-strategy (0.0..=1.0).
    /// The caller multiplies init_cash by this fraction before sizing positions.
    fn equity_fraction(&self) -> f32 {
        1.0
    }

    /// How entry fills are resolved (next-day open, same-day, or 5m confirmation).
    fn fill_mode(&self) -> FillMode {
        FillMode::NextDayOpen
    }

    /// Ordered list of exit rules evaluated each bar while a position is open.
    fn exit_rules(&self, params: &Params) -> Vec<ExitRule>;

    /// Build a universe filter mask for the given date-row range.
    /// Returns a WideMask spanning the full matrix, but only rows in `range`
    /// may be set to true.
    fn filter(&self, store: &DataStore, params: &Params, range: Range<usize>) -> WideMask;

    /// Generate entry/exit/stop signals from the filtered universe mask.
    fn signals(&self, store: &DataStore, universe: &WideMask, params: &Params) -> SignalSet;
}

/// Shared universe filter: price, volume, and ETF exclusion.
///
/// Used by signal_breakout and pattern_breakout (and any future lightweight
/// strategy). The signal pipeline handles selectivity through its composite
/// score; this filter only removes obviously untradeable tickers.
pub fn simple_price_vol_filter(store: &DataStore, params: &Params, range: Range<usize>) -> WideMask {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;
    let mut mask = WideMask::new_false(nr, nc);

    let close_m = store.close();
    let vol_sma = store.get(Indicator::VolSma20);

    for row in range {
        for col in 0..nc {
            if store.axes.etf_cols[col] {
                continue;
            }
            let close = close_m.get(row, col);
            if close.is_nan() || close <= params.min_price {
                continue;
            }
            let vol = vol_sma.get(row, col);
            if vol.is_nan() || vol <= params.min_vol {
                continue;
            }
            mask.set(row, col, true);
        }
    }
    mask
}

/// ATR-capped stop: low-of-day, but no more than 1× ATR below close.
///
/// Falls back to `close * (1 - fallback_pct)` when ATR or low is unavailable.
pub fn atr_stop(close: f32, low: f32, atr: f32, fallback_pct: f32) -> f32 {
    if !atr.is_nan() && !low.is_nan() {
        if (close - low) > atr { close - atr } else { low }
    } else if !low.is_nan() {
        low
    } else {
        close * (1.0 - fallback_pct)
    }
}

/// Factory: map a setup name to its concrete Setup implementations.
///
/// - `"breakout"` produces two sub-strategies (quick profit-taker + runner).
/// - `"ep"` produces the episodic pivot gap setup.
/// - `"parabolic"` produces the parabolic short setup.
/// - Unknown names attempt to load `strategies/{name}.json` as a dynamic pipeline.
/// - If no JSON file found, returns an empty vec (caller should treat as config error).
pub fn create_setups(name: &str) -> Vec<Box<dyn Setup>> {
    match name {
        "breakout" => vec![Box::new(BreakoutQuick), Box::new(BreakoutRunner)],
        "ep" => vec![Box::new(EpisodicPivot)],
        "parabolic" => vec![Box::new(ParabolicShort)],
        "signal_breakout" => vec![Box::new(SignalBreakout)],
        "pattern_breakout" => vec![Box::new(PatternBreakout)],
        other => load_dynamic_setup(other),
    }
}

/// Attempt to load a DynamicSetup from `strategies/{name}.json`.
///
/// Searches relative to the engine crate root (CARGO_MANIFEST_DIR at compile
/// time, falling back to the current working directory at runtime). Returns an
/// empty vec on failure so callers degrade gracefully.
fn load_dynamic_setup(name: &str) -> Vec<Box<dyn Setup>> {
    let candidates = dynamic_strategy_paths(name);
    for path in &candidates {
        if path.exists() {
            match engine_pipeline::DynamicSetup::from_file(path) {
                Ok(inner) => {
                    return vec![Box::new(DynamicSetupAdapter { inner })];
                }
                Err(e) => {
                    eprintln!(
                        "warning: failed to load dynamic strategy {}: {e}",
                        path.display()
                    );
                    return vec![];
                }
            }
        }
    }
    // No config file found -- not necessarily an error; caller decides.
    vec![]
}

/// Build candidate file paths for a dynamic strategy config.
fn dynamic_strategy_paths(name: &str) -> Vec<PathBuf> {
    let filename = format!("{name}.json");
    let mut paths = Vec::with_capacity(2);

    // Compile-time manifest dir (works in tests and cargo run).
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    paths.push(manifest.join("strategies").join(&filename));

    // Runtime cwd fallback (works when invoked from the engine root).
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_path = cwd.join("strategies").join(&filename);
        if !paths.contains(&cwd_path) {
            paths.push(cwd_path);
        }
    }

    paths
}

// ---------------------------------------------------------------------------
// DynamicSetupAdapter: bridges engine_pipeline::DynamicSetup to the Setup trait
// ---------------------------------------------------------------------------

/// Adapter that wraps a pipeline `DynamicSetup` and implements the root crate's
/// `Setup` trait. Handles the type conversions between `DynamicExitRule` and
/// `ExitRule`, and between `FillModeConfig` and `FillMode`.
struct DynamicSetupAdapter {
    inner: engine_pipeline::DynamicSetup,
}

impl Setup for DynamicSetupAdapter {
    fn name(&self) -> &'static str {
        // Strategies are loaded once at startup, so leaking is acceptable.
        Box::leak(self.inner.name().to_string().into_boxed_str())
    }

    fn direction(&self) -> Direction {
        self.inner.direction()
    }

    fn equity_fraction(&self) -> f32 {
        self.inner.equity_fraction()
    }

    fn fill_mode(&self) -> FillMode {
        match self.inner.fill_mode_config() {
            FillModeConfig::NextDayOpen => FillMode::NextDayOpen,
            FillModeConfig::SameDayOpen => FillMode::SameDayOpen,
            FillModeConfig::DualTimeframe => FillMode::DualTimeframe,
        }
    }

    fn exit_rules(&self, _params: &Params) -> Vec<ExitRule> {
        self.inner
            .exit_rules()
            .unwrap_or_default()
            .into_iter()
            .map(convert_exit_rule)
            .collect()
    }

    fn filter(&self, store: &DataStore, _params: &Params, range: Range<usize>) -> WideMask {
        self.inner.run_filter(store, range).unwrap_or_else(|e| {
            eprintln!("DynamicSetup filter error: {e}");
            WideMask::new_false(store.axes.n_rows, store.axes.n_cols)
        })
    }

    fn signals(&self, store: &DataStore, universe: &WideMask, _params: &Params) -> SignalSet {
        self.inner
            .run_signals(store, universe)
            .unwrap_or_else(|e| {
                eprintln!("DynamicSetup signal error: {e}");
                let nr = store.axes.n_rows;
                let nc = store.axes.n_cols;
                SignalSet {
                    entries: WideMask::new_false(nr, nc),
                    exits: WideMask::new_false(nr, nc),
                    stop_prices: WideMatrix::new(vec![f32::NAN; nr * nc], nr, nc),
                }
            })
    }
}

/// Convert a pipeline `DynamicExitRule` to the root crate's `ExitRule`.
fn convert_exit_rule(r: DynamicExitRule) -> ExitRule {
    match r {
        DynamicExitRule::StopLoss => ExitRule::StopLoss,
        DynamicExitRule::ProfitTarget { min_bars } => ExitRule::ProfitTarget { min_bars },
        DynamicExitRule::SmaCross { sma } => ExitRule::SmaCross { sma },
        DynamicExitRule::BreakevenUpgrade { after_bars } => {
            ExitRule::BreakevenUpgrade { after_bars }
        }
        DynamicExitRule::SmaCover { sma1, sma2 } => ExitRule::SmaCover { sma1, sma2 },
        DynamicExitRule::SignalBocpdExit { threshold } => {
            ExitRule::SignalBocpdExit { threshold }
        }
        DynamicExitRule::SignalKalmanStop { atr_mult } => {
            ExitRule::SignalKalmanStop { atr_mult }
        }
    }
}
