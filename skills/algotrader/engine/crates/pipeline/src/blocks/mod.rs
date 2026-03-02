//! Block trait and context for pipeline execution.
//!
//! A `Block` is a stateless pipeline step: it reads from the `Blackboard` and
//! `DataStore`, and produces a `Slot`. Blocks that perform a simple
//! indicator-threshold comparison can declare themselves as fusable via
//! `fusable_spec()`, enabling the optimizer to coalesce consecutive fusable
//! blocks into a single vectorized scan.

use std::collections::HashMap;
use std::ops::Range;

use engine_data::{DataStore, Indicator};

use crate::blackboard::{Blackboard, Slot, SlotType};

pub mod universe;
pub mod pattern;
pub mod entry;
pub mod stop;
pub mod combiner;
pub mod signal;
pub mod cross_sectional;

/// Context passed to every block invocation.
pub struct BlockContext<'a> {
    pub store: &'a DataStore,
    pub config: &'a HashMap<String, serde_json::Value>,
    pub range: Range<usize>,
    pub blackboard: &'a Blackboard,
    pub input_id: Option<&'a str>,
}

impl<'a> BlockContext<'a> {
    /// Get the input mask from the blackboard (from explicit input_id or last step).
    pub fn input_mask(&self) -> Option<&engine_types::WideMask> {
        let slot = if let Some(id) = self.input_id {
            self.blackboard.get(id)
        } else {
            self.blackboard.last()
        };
        slot.and_then(|s| s.as_mask())
    }
}

/// Comparison operator for fusable filter conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Gt,
    Ge,
    Lt,
    Le,
}

/// Describes a simple indicator-threshold filter eligible for fusion.
///
/// When consecutive pipeline steps all have fusable specs, the planner can
/// merge them into a single vectorized pass over the indicator matrices,
/// checking all conditions per cell with short-circuit evaluation.
#[derive(Clone, Debug)]
pub struct FusableSpec {
    pub indicator: Indicator,
    pub op: CmpOp,
    pub threshold_key: String,
}

/// A pipeline block: reads from blackboard + DataStore, produces a Slot.
pub trait Block: Send + Sync {
    fn name(&self) -> &'static str;
    fn output_type(&self) -> SlotType;

    /// Expected input type. Defaults to Mask (most blocks filter an existing mask).
    fn input_type(&self) -> Option<SlotType> {
        Some(SlotType::Mask)
    }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot>;

    /// If this block is a simple indicator-threshold filter, return its spec
    /// so the planner can fuse it with adjacent fusable blocks.
    fn fusable_spec(&self) -> Option<FusableSpec> {
        None
    }
}
