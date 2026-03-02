//! Strategy module: trait-based setup abstraction for backtest strategies.
//!
//! Each setup (breakout, EP, parabolic) implements the `Setup` trait, providing
//! its own universe filter, signal generation, exit rules, and fill mode. The
//! `create_setups()` factory maps a setup name to one or more concrete Setup
//! implementations that the simulation engine runs independently.

mod breakout;
mod ep;
mod parabolic;
mod signal_breakout;

use std::ops::Range;

use crate::data::{DataStore, WideMask};
use crate::execution::{ExitRule, FillMode};
use crate::types::{Direction, Params, SignalSet};

pub use breakout::{BreakoutQuick, BreakoutRunner};
pub use ep::EpisodicPivot;
pub use parabolic::ParabolicShort;
pub use signal_breakout::SignalBreakout;

/// Core abstraction: a trading setup that can filter a universe, generate
/// signals, and declare its execution parameters.
pub trait Setup: Send + Sync {
    /// Human-readable identifier used in trade logs and reports.
    fn name(&self) -> &'static str;

    /// Whether this setup trades long or short.
    fn direction(&self) -> Direction;

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
    fn exit_rules(&self) -> Vec<ExitRule>;

    /// Build a universe filter mask for the given date-row range.
    /// Returns a WideMask spanning the full matrix, but only rows in `range`
    /// may be set to true.
    fn filter(&self, store: &DataStore, params: &Params, range: Range<usize>) -> WideMask;

    /// Generate entry/exit/stop signals from the filtered universe mask.
    fn signals(&self, store: &DataStore, universe: &WideMask, params: &Params) -> SignalSet;
}

/// Factory: map a setup name to its concrete Setup implementations.
///
/// - `"breakout"` produces two sub-strategies (quick profit-taker + runner).
/// - `"ep"` produces the episodic pivot gap setup.
/// - `"parabolic"` produces the parabolic short setup.
/// - Unknown names return an empty vec (caller should treat as a configuration error).
pub fn create_setups(name: &str) -> Vec<Box<dyn Setup>> {
    match name {
        "breakout" => vec![Box::new(BreakoutQuick), Box::new(BreakoutRunner)],
        "ep" => vec![Box::new(EpisodicPivot)],
        "parabolic" => vec![Box::new(ParabolicShort)],
        "signal_breakout" => vec![Box::new(SignalBreakout)],
        _ => vec![],
    }
}
