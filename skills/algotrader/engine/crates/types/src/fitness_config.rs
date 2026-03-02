//! Runtime-configurable constants for fitness evaluation.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessConfig {
    /// Minimum trade count below which fitness is penalized.
    #[serde(default = "default_min_trades_gate")]
    pub min_trades_gate: u64,

    /// Score assigned to penalized (dead) strategies.
    #[serde(default = "default_penalty_score")]
    pub penalty_score: f64,

    /// Base confidence at the minimum trade count.
    #[serde(default = "default_confidence_base")]
    pub confidence_base: f64,

    /// Slope of the linear confidence ramp.
    #[serde(default = "default_confidence_slope")]
    pub confidence_slope: f64,

    /// Trade count range over which confidence ramps from base to 1.0.
    #[serde(default = "default_confidence_range")]
    pub confidence_range: f64,

    /// Base Sharpe cap (grows with log10 of trade count).
    #[serde(default = "default_sharpe_cap_base")]
    pub sharpe_cap_base: f64,

    /// Weight of Sharpe in the composite fitness formula.
    #[serde(default = "default_composite_w_sharpe")]
    pub composite_w_sharpe: f64,

    /// Weight of return in the composite fitness formula.
    #[serde(default = "default_composite_w_return")]
    pub composite_w_return: f64,

    /// Weight of win rate in the composite fitness formula.
    #[serde(default = "default_composite_w_winrate")]
    pub composite_w_winrate: f64,

    /// Scaling factor applied to raw return before weighting.
    #[serde(default = "default_composite_return_scale")]
    pub composite_return_scale: f64,

    /// Multiplier for drawdown in the penalty calculation (dd * mult).
    #[serde(default = "default_drawdown_penalty_mult")]
    pub drawdown_penalty_mult: f64,
}

fn default_min_trades_gate() -> u64 { 10 }
fn default_penalty_score() -> f64 { -999.0 }
fn default_confidence_base() -> f64 { 0.3 }
fn default_confidence_slope() -> f64 { 0.7 }
fn default_confidence_range() -> f64 { 40.0 }
fn default_sharpe_cap_base() -> f64 { 3.5 }
fn default_composite_w_sharpe() -> f64 { 0.4 }
fn default_composite_w_return() -> f64 { 0.3 }
fn default_composite_w_winrate() -> f64 { 0.3 }
fn default_composite_return_scale() -> f64 { 10.0 }
fn default_drawdown_penalty_mult() -> f64 { 2.0 }

impl Default for FitnessConfig {
    fn default() -> Self {
        Self {
            min_trades_gate: default_min_trades_gate(),
            penalty_score: default_penalty_score(),
            confidence_base: default_confidence_base(),
            confidence_slope: default_confidence_slope(),
            confidence_range: default_confidence_range(),
            sharpe_cap_base: default_sharpe_cap_base(),
            composite_w_sharpe: default_composite_w_sharpe(),
            composite_w_return: default_composite_w_return(),
            composite_w_winrate: default_composite_w_winrate(),
            composite_return_scale: default_composite_return_scale(),
            drawdown_penalty_mult: default_drawdown_penalty_mult(),
        }
    }
}
