//! Fuzzy pattern recognition for OHLCV price series.
//!
//! Each pattern is scored as a continuous confidence value in [0, 1]
//! using fuzzy membership functions (sigmoid, Cauchy proxy for Gaussian,
//! trapezoidal). No lookahead — all signals are causal.
//!
//! Patterns:
//! - `bull_flag::bull_flag_confidence()` — scores setup quality *before* a breakout
//! - `breakout_detect::breakout_detected()` — scores whether a breakout *already happened*

pub mod breakout_detect;
pub mod bull_flag;
pub mod membership;
pub mod scorer;

pub use breakout_detect::breakout_detected;
pub use bull_flag::bull_flag_confidence;
pub use scorer::compute_pattern_score;
