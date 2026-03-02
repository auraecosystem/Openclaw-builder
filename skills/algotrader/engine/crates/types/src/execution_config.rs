//! Runtime-configurable constants for execution (position sizing, slippage, holds).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Fraction of daily volume used as liquidity cap for position sizing.
    #[serde(default = "default_liquidity_cap_coeff")]
    pub liquidity_cap_coeff: f32,

    /// Base slippage percentage applied to every fill.
    #[serde(default = "default_slippage_base")]
    pub slippage_base: f32,

    /// Maximum slippage percentage (caps the volume-impact model).
    #[serde(default = "default_slippage_max")]
    pub slippage_max: f32,

    /// Fallback slippage when volume data is missing.
    #[serde(default = "default_slippage_fallback")]
    pub slippage_fallback: f32,

    /// Minimum number of shares/units per position.
    #[serde(default = "default_min_shares")]
    pub min_shares: f32,

    /// Minimum bars a position must be held before exits are evaluated.
    #[serde(default = "default_min_hold_bars")]
    pub min_hold_bars: u32,
}

fn default_liquidity_cap_coeff() -> f32 { 0.01 }
fn default_slippage_base() -> f32 { 0.001 }
fn default_slippage_max() -> f32 { 0.05 }
fn default_slippage_fallback() -> f32 { 0.001 }
fn default_min_shares() -> f32 { 1.0 }
fn default_min_hold_bars() -> u32 { 3 }

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            liquidity_cap_coeff: default_liquidity_cap_coeff(),
            slippage_base: default_slippage_base(),
            slippage_max: default_slippage_max(),
            slippage_fallback: default_slippage_fallback(),
            min_shares: default_min_shares(),
            min_hold_bars: default_min_hold_bars(),
        }
    }
}
