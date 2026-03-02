//! Runtime-configurable constants for strategy logic (signal_breakout setup).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct StrategyConfig {
    /// Minimum bars before profit-target exit is eligible.
    pub profit_target_min_bars: u32,

    /// Minimum bars of data required before generating signal-based entries.
    pub min_signal_data: u32,

    /// Fallback stop-loss distance as fraction of entry price (when ATR unavailable).
    pub stop_fallback_pct: f32,

    // --- Parabolic short thresholds -----------------------------------------

    /// Price threshold above which a stock is treated as "large cap"
    /// for the parabolic short run-size requirement.
    pub large_cap_price: f32,

    /// Minimum 10-day gain for a large-cap parabolic short candidate.
    pub large_cap_run: f32,

    /// Minimum 10-day gain for a small-cap parabolic short candidate.
    pub small_cap_run: f32,

    // --- Episodic Pivot thresholds ------------------------------------------

    /// Minimum gap-day dollar volume for EP entries.
    pub ep_min_gap_day_dollar_vol: f32,

    /// Minimum gap percentage (open / prev_close - 1) for EP entries.
    pub ep_min_gap_pct: f32,

    /// Minimum volume multiple vs 20-day SMA for EP entries.
    pub ep_min_vol_ratio: f32,

    /// Maximum 6-month prior return for EP "neglect" filter.
    pub ep_max_prior_6m_return: f32,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            profit_target_min_bars: 5,
            min_signal_data: 30,
            stop_fallback_pct: 0.05,
            large_cap_price: 50.0,
            large_cap_run: 0.50,
            small_cap_run: 3.00,
            ep_min_gap_day_dollar_vol: 100_000_000.0,
            ep_min_gap_pct: 0.10,
            ep_min_vol_ratio: 2.0,
            ep_max_prior_6m_return: 0.30,
        }
    }
}
