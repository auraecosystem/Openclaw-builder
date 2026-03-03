//! JIT compilation for fused filter conditions using Cranelift.
//!
//! Compiles pipeline filter conditions into native branchless code at runtime.
//! Thresholds are runtime arguments (CMA-ES varies them without recompilation).
//! When block structure changes (toggling steps on/off), the cache maps each
//! unique condition signature to a compiled function.

pub mod cache;
pub mod compiler;

pub use cache::JitCache;
pub use compiler::{compile_filter, CmpOp, JitCondition, JitFilter};
