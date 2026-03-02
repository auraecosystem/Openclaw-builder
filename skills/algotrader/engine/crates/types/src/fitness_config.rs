//! Runtime-configurable constants for fitness evaluation.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FitnessConfig {
    /// Minimum trade count below which fitness is penalized.
    pub min_trades_gate: u64,

    /// Score assigned to penalized (dead) strategies.
    pub penalty_score: f64,

    /// Base confidence at the minimum trade count.
    pub confidence_base: f64,

    /// Slope of the linear confidence ramp.
    pub confidence_slope: f64,

    /// Trade count range over which confidence ramps from base to 1.0.
    pub confidence_range: f64,

    /// Base Sharpe cap (grows with log10 of trade count).
    pub sharpe_cap_base: f64,

    /// Weight of Sharpe in the composite fitness formula.
    pub composite_w_sharpe: f64,

    /// Weight of return in the composite fitness formula.
    pub composite_w_return: f64,

    /// Weight of win rate in the composite fitness formula.
    pub composite_w_winrate: f64,

    /// Scaling factor applied to raw return before weighting.
    pub composite_return_scale: f64,

    /// Multiplier for drawdown in the penalty calculation (dd * mult).
    pub drawdown_penalty_mult: f64,
}

impl Default for FitnessConfig {
    fn default() -> Self {
        Self {
            min_trades_gate: 10,
            penalty_score: -999.0,
            confidence_base: 0.3,
            confidence_slope: 0.7,
            confidence_range: 40.0,
            sharpe_cap_base: 3.5,
            composite_w_sharpe: 0.4,
            composite_w_return: 0.3,
            composite_w_winrate: 0.3,
            composite_return_scale: 10.0,
            drawdown_penalty_mult: 2.0,
        }
    }
}
