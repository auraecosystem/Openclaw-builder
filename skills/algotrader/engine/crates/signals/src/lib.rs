//! Signal processing crate: composable algorithm pipeline from cheap
//! scanning to expensive investigation to execution signals.
//!
//! See `SIGNAL_SPEC.md` for implementation contracts and
//! `../signal-stack.md` for the full algorithm reference.

pub mod algorithms;
pub mod arena;
pub mod execution_signals;
pub mod pipeline;
pub mod scorer;

// Re-export key types so callers can use short paths.
pub use arena::ThreadArena;
pub use execution_signals::ExecutionSignals;
pub use pipeline::{
    CharacterizationState, InvestigationResult, RawAlgorithmOutputs, SignalOutput,
    run_pattern_pipeline,
};

// Re-export types from engine-types that are part of the signals public API.
pub use engine_types::{SignalParams, SCANNER_FEATURE_COUNT};

// Backward-compatible re-exports so paths like `signals::params::SignalParams`
// and `signals::feature::SCANNER_FEATURE_COUNT` continue to resolve.
pub use engine_types::signal_params as params;
pub use engine_types::feature;
