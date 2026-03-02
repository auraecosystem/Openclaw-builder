//! Signal processing module -- 4-layer funnel from cheap scanner to
//! expensive agent investigation to execution signals.
//!
//! See `SIGNAL_SPEC.md` for implementation contracts and
//! `../signal-stack.md` for the full algorithm reference.

pub mod arena;
pub mod feature;
pub mod layer0;
pub mod layer1;
pub mod layer2;
pub mod layer3;
pub mod params;
pub mod pipeline;

// Re-export the key types so callers can `use crate::signals::SignalParams`
// without navigating submodules.
pub use arena::ThreadArena;
pub use feature::SCANNER_FEATURE_COUNT;
pub use layer0::CharacterizationState;
pub use layer1::ScannerOutput;
pub use layer2::InvestigationResult;
pub use layer3::ExecutionSignals;
pub use params::SignalParams;
pub use pipeline::SignalOutput;
