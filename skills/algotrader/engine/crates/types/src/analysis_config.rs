//! Runtime-configurable constants for analysis and reporting.
//!
//! `intraday_bars_per_day` is used as a fallback for DSR calculation
//! when DataConfig is not directly available. Prefer computing this from
//! DataConfig.intraday_bars_per_day() when possible.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisConfig {
    /// Intraday bars per calendar day (288 for 5m crypto, 78 for US equities).
    pub intraday_bars_per_day: f64,
    /// When true, include the equity curve in the Report JSON output.
    /// Disabled by default to keep single-run output compact.
    /// Always computed internally for portfolio correlation.
    pub emit_equity_curve: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            intraday_bars_per_day: 288.0,
            emit_equity_curve: false,
        }
    }
}
