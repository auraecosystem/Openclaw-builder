//! Runtime-configurable constants for execution (position sizing, slippage, holds).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ExecutionConfig {
    /// Fraction of daily volume used as liquidity cap for position sizing.
    pub liquidity_cap_coeff: f32,

    /// Base slippage percentage applied to every fill.
    pub slippage_base: f32,

    /// Maximum slippage percentage (caps the volume-impact model).
    pub slippage_max: f32,

    /// Fallback slippage when volume data is missing.
    pub slippage_fallback: f32,

    /// Minimum number of shares/units per position (1.0 for stocks, 0.001 for crypto).
    pub min_shares: f32,

    /// Minimum bars a position must be held before exits are evaluated.
    pub min_hold_bars: u32,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            liquidity_cap_coeff: 0.01,
            slippage_base: 0.001,
            slippage_max: 0.05,
            slippage_fallback: 0.001,
            min_shares: 1.0,
            min_hold_bars: 3,
        }
    }
}
