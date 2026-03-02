//! Analysis module: financial metrics and backtest reporting.

pub use engine_types::analysis_config as config;
pub mod metrics;
pub mod report;

pub use report::{generate_report, Report};
