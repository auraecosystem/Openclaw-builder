//! Runtime-configurable constants for strategy logic (signal_breakout setup).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// Minimum bars before profit-target exit is eligible.
    #[serde(default = "default_profit_target_min_bars")]
    pub profit_target_min_bars: u32,

    /// Minimum bars of data required before generating signal-based entries.
    #[serde(default = "default_min_signal_data")]
    pub min_signal_data: u32,

    /// Fallback stop-loss distance as fraction of entry price (when ATR unavailable).
    #[serde(default = "default_stop_fallback_pct")]
    pub stop_fallback_pct: f32,
}

fn default_profit_target_min_bars() -> u32 { 5 }
fn default_min_signal_data() -> u32 { 30 }
fn default_stop_fallback_pct() -> f32 { 0.05 }

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            profit_target_min_bars: default_profit_target_min_bars(),
            min_signal_data: default_min_signal_data(),
            stop_fallback_pct: default_stop_fallback_pct(),
        }
    }
}
