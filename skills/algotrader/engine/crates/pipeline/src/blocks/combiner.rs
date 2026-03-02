//! Mask combiner blocks: And, Or, Not.
//!
//! These blocks operate on `WideMask` intermediates, combining or inverting
//! boolean masks produced by earlier pipeline steps. `And` and `Or` read a
//! second mask from the blackboard by step ID (`config["other"]`).

use engine_types::WideMask;

use crate::blackboard::{Slot, SlotType};
use super::{Block, BlockContext};

// ---------------------------------------------------------------------------
// And
// ---------------------------------------------------------------------------

/// Element-wise AND of the implicit input mask and a named blackboard mask.
///
/// Config: `"other"` (string) -- step ID of the second mask on the blackboard.
pub struct And;

impl Block for And {
    fn name(&self) -> &'static str { "and" }
    fn output_type(&self) -> SlotType { SlotType::Mask }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let other_id = ctx.config.get("other")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("and: missing 'other' step ID in config"))?;

        let a = ctx.input_mask()
            .ok_or_else(|| anyhow::anyhow!("and: no input mask"))?;
        let b = ctx.blackboard.get(other_id)
            .and_then(|s| s.as_mask())
            .ok_or_else(|| anyhow::anyhow!("and: '{other_id}' not found or not a mask"))?;

        let len = a.data.len();
        anyhow::ensure!(
            len == b.data.len(),
            "and: mask length mismatch ({len} vs {})", b.data.len()
        );

        let mut result = WideMask::new_false(a.n_rows(), a.n_cols());
        for i in 0..len {
            result.data[i] = a.data[i] && b.data[i];
        }
        Ok(Slot::Mask(result))
    }
}

// ---------------------------------------------------------------------------
// Or
// ---------------------------------------------------------------------------

/// Element-wise OR of the implicit input mask and a named blackboard mask.
///
/// Config: `"other"` (string) -- step ID of the second mask on the blackboard.
pub struct Or;

impl Block for Or {
    fn name(&self) -> &'static str { "or" }
    fn output_type(&self) -> SlotType { SlotType::Mask }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let other_id = ctx.config.get("other")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("or: missing 'other' step ID in config"))?;

        let a = ctx.input_mask()
            .ok_or_else(|| anyhow::anyhow!("or: no input mask"))?;
        let b = ctx.blackboard.get(other_id)
            .and_then(|s| s.as_mask())
            .ok_or_else(|| anyhow::anyhow!("or: '{other_id}' not found or not a mask"))?;

        let len = a.data.len();
        anyhow::ensure!(
            len == b.data.len(),
            "or: mask length mismatch ({len} vs {})", b.data.len()
        );

        let mut result = WideMask::new_false(a.n_rows(), a.n_cols());
        for i in 0..len {
            result.data[i] = a.data[i] || b.data[i];
        }
        Ok(Slot::Mask(result))
    }
}

// ---------------------------------------------------------------------------
// Not
// ---------------------------------------------------------------------------

/// Element-wise inversion of the implicit input mask.
pub struct Not;

impl Block for Not {
    fn name(&self) -> &'static str { "not" }
    fn output_type(&self) -> SlotType { SlotType::Mask }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let a = ctx.input_mask()
            .ok_or_else(|| anyhow::anyhow!("not: no input mask"))?;

        let mut result = WideMask::new_false(a.n_rows(), a.n_cols());
        for i in 0..a.data.len() {
            result.data[i] = !a.data[i];
        }
        Ok(Slot::Mask(result))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use engine_data::{DataStore, Axes};
    use engine_types::WideMatrix;
    use crate::blackboard::Blackboard;

    /// Matches strum EnumCount for Indicator without adding strum as a dep.
    use strum::EnumCount;
    const INDICATOR_COUNT: usize = engine_data::Indicator::COUNT;

    /// Minimal DataStore for tests that never read indicator data.
    fn dummy_store() -> DataStore {
        let (nr, nc) = (2, 2);
        let daily: Vec<WideMatrix> = (0..INDICATOR_COUNT)
            .map(|_| WideMatrix::new(vec![0.0; nr * nc], nr, nc))
            .collect();
        let axes = Axes {
            dates: vec![0; nr],
            tickers: vec!["A".into(), "B".into()],
            ticker_idx: [("A".into(), 0), ("B".into(), 1)].into(),
            spy_col: None,
            etf_cols: vec![false; nc],
            n_rows: nr,
            n_cols: nc,
            trading_hours: 6.5,
        };
        DataStore::new(axes, daily, None)
    }

    /// Build a 2x2 WideMask from a 4-element bool slice.
    fn mask_2x2(vals: [bool; 4]) -> WideMask {
        let mut m = WideMask::new_false(2, 2);
        for (i, &v) in vals.iter().enumerate() {
            m.data[i] = v;
        }
        m
    }

    /// Helper: run a combiner block with two masks on the blackboard.
    fn run_binary(
        block: &dyn Block,
        input: WideMask,
        other: WideMask,
    ) -> anyhow::Result<Slot> {
        let store = dummy_store();
        let mut bb = Blackboard::new();
        bb.put("input_step".into(), Slot::Mask(input));
        bb.put("other_step".into(), Slot::Mask(other));

        let mut config = HashMap::new();
        config.insert("other".into(), serde_json::json!("other_step"));

        let ctx = BlockContext {
            store: &store,
            config: &config,
            range: 0..2,
            blackboard: &bb,
            input_id: Some("input_step"),
        };
        block.execute(&ctx)
    }

    #[test]
    fn and_basic() {
        let a = mask_2x2([true, false, true, true]);
        let b = mask_2x2([true, true, false, true]);
        let result = run_binary(&And, a, b).unwrap();
        let m = result.as_mask().unwrap();
        assert_eq!(m.data, vec![true, false, false, true]);
    }

    #[test]
    fn or_basic() {
        let a = mask_2x2([true, false, false, true]);
        let b = mask_2x2([false, true, false, true]);
        let result = run_binary(&Or, a, b).unwrap();
        let m = result.as_mask().unwrap();
        assert_eq!(m.data, vec![true, true, false, true]);
    }

    #[test]
    fn not_basic() {
        let store = dummy_store();
        let input = mask_2x2([true, false, true, false]);
        let mut bb = Blackboard::new();
        bb.put("prev".into(), Slot::Mask(input));

        let ctx = BlockContext {
            store: &store,
            config: &HashMap::new(),
            range: 0..2,
            blackboard: &bb,
            input_id: Some("prev"),
        };
        let result = Not.execute(&ctx).unwrap();
        let m = result.as_mask().unwrap();
        assert_eq!(m.data, vec![false, true, false, true]);
    }

    #[test]
    fn and_missing_other_errors() {
        let store = dummy_store();
        let mut bb = Blackboard::new();
        bb.put("s".into(), Slot::Mask(mask_2x2([true; 4])));

        let ctx = BlockContext {
            store: &store,
            config: &HashMap::new(),
            range: 0..2,
            blackboard: &bb,
            input_id: Some("s"),
        };
        assert!(And.execute(&ctx).is_err());
    }
}
