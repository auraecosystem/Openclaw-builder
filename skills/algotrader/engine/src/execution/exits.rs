//! Exit rules and context for the simulation loop.
//!
//! Each `ExitRule` variant encodes one reason to close a position. The generic
//! simulator iterates an `&[ExitRule]` slice every bar, short-circuiting on
//! the first rule that fires. `update_stop()` runs unconditionally each bar
//! so trailing/breakeven logic can upgrade the active stop before exit checks.

use crate::data::{DataStore, Indicator};
use crate::types::Direction;

/// Per-bar snapshot passed to every exit rule evaluation.
pub struct ExitContext {
    pub row: usize,
    pub col: usize,
    pub close: f32,
    pub entry_price: f32,
    pub bars_since_entry: usize,
    pub active_stop: f32,
    pub direction: Direction,
}

#[derive(Clone, Debug)]
pub enum ExitRule {
    /// Exit when close crosses below (long) or above (short) an SMA indicator.
    /// Used by breakout quick/runner (Sma10) for long trend-following exits.
    SmaCross { sma: Indicator },

    /// Take profit after holding at least `min_bars` if position is profitable.
    /// Long only -- used by breakout quick (5 bars).
    ProfitTarget { min_bars: usize },

    /// Upgrade stop to entry price (breakeven) after `after_bars` if profitable.
    /// Never triggers an exit directly -- only modifies the active stop via
    /// `update_stop()`. Used by breakout runner.
    BreakevenUpgrade { after_bars: usize },

    /// Exit when close breaches the active stop level.
    /// Long: close <= stop. Short: close >= stop. Also checks intraday 5m stops
    /// (handled separately by the simulator before calling `should_exit`).
    StopLoss,

    /// Cover a short when close drops below either SMA.
    /// Used by parabolic short strategy.
    SmaCover { sma1: Indicator, sma2: Indicator },

    /// Exit when BOCPD detects regime death (P(changepoint) > threshold).
    /// The actual BOCPD check runs in `SignalBreakout::signals()` and sets the
    /// exit mask; at simulation time this behaves like `StopLoss` (checks the
    /// pre-computed exit mask via the standard exit flow).
    SignalBocpdExit { threshold: f32 },

    /// Dynamic Kalman trailing stop: exit when close < kalman_level - mult * ATR.
    /// The actual stop computation happens in `SignalBreakout::signals()` and is
    /// stored in `stop_prices`; this rule checks against `active_stop` the same
    /// way `StopLoss` does.
    SignalKalmanStop { atr_mult: f32 },
}

impl ExitRule {
    /// Returns true if this rule triggers an exit for the given bar context.
    #[inline]
    pub fn should_exit(&self, ctx: &ExitContext, store: &DataStore) -> bool {
        match self {
            Self::SmaCross { sma } => {
                let sma_val = store.get(*sma).get(ctx.row, ctx.col);
                if sma_val.is_nan() {
                    return false;
                }
                match ctx.direction {
                    Direction::Long => ctx.close < sma_val,
                    Direction::Short => ctx.close > sma_val,
                }
            }

            Self::ProfitTarget { min_bars } => {
                // Long only: exit after min_bars if profitable
                ctx.bars_since_entry >= *min_bars && ctx.close > ctx.entry_price
            }

            Self::StopLoss => {
                if ctx.active_stop.is_nan() {
                    return false;
                }
                match ctx.direction {
                    Direction::Long => ctx.close <= ctx.active_stop,
                    Direction::Short => ctx.close >= ctx.active_stop,
                }
            }

            Self::SmaCover { sma1, sma2 } => {
                // Cover short when price drops below either SMA
                let v1 = store.get(*sma1).get(ctx.row, ctx.col);
                let v2 = store.get(*sma2).get(ctx.row, ctx.col);
                let below1 = !v1.is_nan() && ctx.close <= v1;
                let below2 = !v2.is_nan() && ctx.close <= v2;
                below1 || below2
            }

            // BreakevenUpgrade never exits -- it only modifies the stop
            Self::BreakevenUpgrade { .. } => false,

            // Signal-based exit rules: the actual signal logic runs in the
            // strategy's signals() method and writes into stop_prices/exits.
            // At simulation time these check the stop like StopLoss does.
            Self::SignalBocpdExit { .. } | Self::SignalKalmanStop { .. } => {
                if ctx.active_stop.is_nan() {
                    return false;
                }
                match ctx.direction {
                    Direction::Long => ctx.close <= ctx.active_stop,
                    Direction::Short => ctx.close >= ctx.active_stop,
                }
            }
        }
    }

    /// Returns `Some(new_stop)` if this rule wants to tighten the trailing stop.
    /// Called every bar regardless of whether `should_exit` fired.
    #[inline]
    pub fn update_stop(&self, ctx: &ExitContext) -> Option<f32> {
        match self {
            Self::BreakevenUpgrade { after_bars } => {
                if ctx.bars_since_entry >= *after_bars && ctx.close > ctx.entry_price {
                    // Upgrade stop to at least entry price (breakeven)
                    Some(f32::max(ctx.active_stop, ctx.entry_price))
                } else {
                    None
                }
            }
            // Signal-based exit rules do not modify the stop at simulation
            // time; dynamic stops are pre-computed in signals().
            Self::SignalBocpdExit { .. } | Self::SignalKalmanStop { .. } => None,

            // Other rules do not modify the stop
            _ => None,
        }
    }
}
