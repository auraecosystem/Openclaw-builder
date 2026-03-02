//! Leaf types crate: data structures, configuration, and constants shared
//! across the algotrader engine workspace. No heavy dependencies -- only serde.

pub mod analysis_config;
pub mod cmaes_config;
pub mod execution_config;
pub mod feature;
pub mod fitness_config;
pub mod ga_config;
pub mod matrix;
pub mod pattern;
pub mod signal_params;
pub mod strategy_config;
pub mod types;

// Re-export key types at crate root for ergonomic imports.
pub use analysis_config::AnalysisConfig;
pub use cmaes_config::CmaEsConfig;
pub use execution_config::ExecutionConfig;
pub use feature::{FeatureVec, SCANNER_FEATURE_COUNT};
pub use fitness_config::FitnessConfig;
pub use ga_config::GaConfig;
pub use matrix::{WideMask, WideMatrix};
pub use pattern::{BreakoutDetectParams, PatternParams, PatternScores, PATTERN_FEATURE_COUNT};
pub use signal_params::SignalParams;
pub use strategy_config::StrategyConfig;
pub use types::{Direction, Params, ResolvedParams, SignalSet, Trade};
