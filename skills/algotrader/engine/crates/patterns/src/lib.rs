//! Fuzzy pattern recognition for OHLCV price series.
//!
//! Each pattern is scored as a continuous confidence value in [0, 1]
//! using fuzzy membership functions (sigmoid, Cauchy proxy for Gaussian,
//! trapezoidal). No lookahead — all signals are causal.
//!
//! Entry point: `bull_flag::bull_flag_confidence()`.

pub mod bull_flag;
pub mod membership;
pub mod scorer;

pub use bull_flag::bull_flag_confidence;
pub use scorer::compute_pattern_score;
