//! Analysis module: financial metrics and backtest reporting.

pub mod metrics;
pub mod report;

pub use report::{generate_report, Report};
