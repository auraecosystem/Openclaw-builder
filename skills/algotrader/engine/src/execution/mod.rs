//! Execution module: position sizing, fill resolution, exit rules, and the
//! generic simulation loop.
//!
//! This module replaces the three near-identical simulation loops from the
//! original simulate.rs with one parameterized `simulate()` function.

pub use engine_types::execution_config as config;
mod exits;
mod fills;
mod position;
mod simulator;

pub use exits::{ExitContext, ExitRule};
pub use fills::{check_5m_stop, resolve_fill, FillMode};
pub use position::PositionSizer;
pub use simulator::simulate;
