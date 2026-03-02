//! Runtime-configurable constants for analysis and reporting.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Number of intraday bars per calendar day (288 for 5m crypto, 78 for US equities).
    #[serde(default = "default_intraday_bars_per_day")]
    pub intraday_bars_per_day: f64,
}

fn default_intraday_bars_per_day() -> f64 { 288.0 }

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            intraday_bars_per_day: default_intraday_bars_per_day(),
        }
    }
}
