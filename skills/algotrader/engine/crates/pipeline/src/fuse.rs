//! FusedFilter: coalesce consecutive simple indicator-threshold filters.
//!
//! When consecutive pipeline steps each compare a single indicator value
//! against a threshold (no when-conditional, no explicit input ref), they
//! can be merged into a single `FusedFilter` that makes one pass over the
//! matrix, checking all conditions per cell with short-circuit evaluation.
//! This reproduces the performance of handwritten fused filters like
//! `breakout_filter` in `strategy/breakout.rs`.

use std::ops::Range;

use engine_data::{DataStore, Indicator};
use engine_types::WideMask;

use crate::blocks::CmpOp;
use crate::plan::{LogicalStep, PhysicalStep};
use crate::registry::BlockRegistry;

// ---------------------------------------------------------------------------
// FusedCondition + FusedFilter
// ---------------------------------------------------------------------------

/// A single indicator-threshold condition within a fused filter.
///
/// The threshold is a concrete f32, already resolved from @param references
/// by the parse phase.
#[derive(Clone, Debug)]
pub struct FusedCondition {
    pub indicator: Indicator,
    pub op: CmpOp,
    pub threshold: f32,
}

impl FusedCondition {
    /// Evaluate the condition on a single cell value.
    #[inline]
    fn check(&self, value: f32) -> bool {
        if value.is_nan() {
            return false;
        }
        match self.op {
            CmpOp::Gt => value > self.threshold,
            CmpOp::Ge => value >= self.threshold,
            CmpOp::Lt => value < self.threshold,
            CmpOp::Le => value <= self.threshold,
        }
    }
}

/// Multiple indicator-threshold conditions fused into one vectorized scan.
///
/// Pre-fetches all indicator matrices, then makes a single pass checking
/// every condition per cell with short-circuit on first failure.
#[derive(Clone, Debug)]
pub struct FusedFilter {
    pub conditions: Vec<FusedCondition>,
    pub step_id: String,
}

impl FusedFilter {
    /// Execute the fused filter over the given range, masking against `input`.
    ///
    /// For each (row, col) in `range` where `input[row][col]` is true, checks
    /// all conditions with short-circuit. If any condition fails, the output
    /// cell is false.
    pub fn execute(
        &self,
        store: &DataStore,
        input: &WideMask,
        range: Range<usize>,
    ) -> WideMask {
        let nr = store.axes.n_rows;
        let nc = store.axes.n_cols;

        // Pre-fetch all indicator matrices to avoid repeated lookups.
        let matrices: Vec<&engine_types::WideMatrix> = self
            .conditions
            .iter()
            .map(|c| store.get(c.indicator))
            .collect();

        let mut output = WideMask::new_false(nr, nc);

        for row in range {
            for col in 0..nc {
                // Skip cells not in the input mask.
                if !input.get(row, col) {
                    continue;
                }

                // Short-circuit: fail fast on first condition that doesn't pass.
                let pass = self
                    .conditions
                    .iter()
                    .zip(matrices.iter())
                    .all(|(cond, mat)| {
                        let v = mat.get(row, col);
                        cond.check(v)
                    });

                if pass {
                    output.set(row, col, true);
                }
            }
        }

        output
    }
}

// ---------------------------------------------------------------------------
// fuse_steps: scan LogicalSteps and coalesce fusable runs
// ---------------------------------------------------------------------------

/// Scan consecutive logical steps and coalesce fusable runs into FusedFilters.
///
/// A step is eligible for fusion when:
/// - The block has `fusable_spec().is_some()`
/// - The step has no `when` conditional
/// - The step has no explicit `input` reference
/// - The threshold value can be resolved from the step's config map
///
/// Non-fusable steps break the run. The result is a mixed vec of Fused and
/// Single physical steps.
pub fn fuse_steps(
    logical_steps: &[LogicalStep],
    registry: &BlockRegistry,
) -> Vec<PhysicalStep> {
    let mut result = Vec::new();
    let mut pending_conditions: Vec<FusedCondition> = Vec::new();
    let mut pending_ids: Vec<String> = Vec::new();

    for ls in logical_steps {
        // Check if this step can be fused.
        let can_fuse = ls.when.is_none()
            && ls.input.is_none()
            && try_build_condition(ls, registry).is_some();

        if can_fuse {
            let cond = try_build_condition(ls, registry).unwrap();
            pending_conditions.push(cond);
            pending_ids.push(ls.step_id.clone());
        } else {
            // Non-fusable step: flush any accumulated fused conditions first.
            flush_pending(&mut pending_conditions, &mut pending_ids, &mut result);

            // Emit the non-fusable step as Single.
            result.push(PhysicalStep::Single {
                block_name: ls.block_name.clone(),
                config: ls.config.clone(),
                step_id: ls.step_id.clone(),
                input_id: ls.input.clone(),
                when: ls.when.clone(),
            });
        }
    }

    // Flush any trailing fused conditions.
    flush_pending(&mut pending_conditions, &mut pending_ids, &mut result);

    result
}

/// Try to build a FusedCondition from a logical step.
///
/// Returns None if the block is not fusable or the threshold cannot be
/// resolved from the step's config map.
fn try_build_condition(
    ls: &LogicalStep,
    registry: &BlockRegistry,
) -> Option<FusedCondition> {
    let block = registry.get(&ls.block_name)?;
    let spec = block.fusable_spec()?;

    // Look up the threshold value from the step's config using the key
    // declared by the FusableSpec.
    let threshold = ls
        .config
        .get(&spec.threshold_key)
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)?;

    Some(FusedCondition {
        indicator: spec.indicator,
        op: spec.op,
        threshold,
    })
}

/// Flush accumulated fusable conditions into the result vec.
///
/// A single condition still becomes a FusedFilter (1 condition is fine).
fn flush_pending(
    conditions: &mut Vec<FusedCondition>,
    ids: &mut Vec<String>,
    result: &mut Vec<PhysicalStep>,
) {
    if conditions.is_empty() {
        return;
    }

    let step_id = if ids.len() == 1 {
        ids[0].clone()
    } else {
        // Composite ID for debugging: "fused_s0_s1_s2"
        format!("fused_{}", ids.join("_"))
    };

    result.push(PhysicalStep::Fused(FusedFilter {
        conditions: std::mem::take(conditions),
        step_id,
    }));

    ids.clear();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blackboard::{Slot, SlotType};
    use crate::blocks::{Block, BlockContext, FusableSpec};
    use engine_data::Indicator;
    use engine_types::WideMatrix;

    // -- Mock blocks -------------------------------------------------------

    struct MockFusable {
        type_name: &'static str,
        indicator: Indicator,
        op: CmpOp,
        threshold_key: &'static str,
    }

    impl Block for MockFusable {
        fn name(&self) -> &'static str { self.type_name }
        fn output_type(&self) -> SlotType { SlotType::Mask }
        fn input_type(&self) -> Option<SlotType> { None }
        fn execute(&self, _ctx: &BlockContext) -> anyhow::Result<Slot> {
            Ok(Slot::Mask(WideMask::new_false(1, 1)))
        }
        fn fusable_spec(&self) -> Option<FusableSpec> {
            Some(FusableSpec {
                indicator: self.indicator,
                op: self.op,
                threshold_key: self.threshold_key.to_string(),
            })
        }
    }

    struct MockNonFusable(&'static str);

    impl Block for MockNonFusable {
        fn name(&self) -> &'static str { self.0 }
        fn output_type(&self) -> SlotType { SlotType::Mask }
        fn input_type(&self) -> Option<SlotType> { None }
        fn execute(&self, _ctx: &BlockContext) -> anyhow::Result<Slot> {
            Ok(Slot::Mask(WideMask::new_false(1, 1)))
        }
    }

    // -- Helpers -----------------------------------------------------------

    fn fusable_registry() -> BlockRegistry {
        let mut reg = BlockRegistry::new();
        reg.register(Box::new(MockFusable {
            type_name: "price_floor",
            indicator: Indicator::Close,
            op: CmpOp::Gt,
            threshold_key: "min",
        }));
        reg.register(Box::new(MockFusable {
            type_name: "volume_floor",
            indicator: Indicator::VolSma20,
            op: CmpOp::Gt,
            threshold_key: "min",
        }));
        reg.register(Box::new(MockFusable {
            type_name: "near_52w_high",
            indicator: Indicator::Dist52w,
            op: CmpOp::Le,
            threshold_key: "max_dist",
        }));
        reg.register(Box::new(MockFusable {
            type_name: "prior_move",
            indicator: Indicator::Ret63,
            op: CmpOp::Ge,
            threshold_key: "min",
        }));
        reg.register(Box::new(MockFusable {
            type_name: "extra",
            indicator: Indicator::Atr14,
            op: CmpOp::Gt,
            threshold_key: "min",
        }));
        reg.register(Box::new(MockNonFusable("consolidation")));
        reg
    }

    fn logical(
        block_name: &str,
        id: &str,
        config: Vec<(&str, f32)>,
    ) -> LogicalStep {
        let mut cfg = std::collections::HashMap::new();
        for (k, v) in config {
            cfg.insert(k.to_string(), serde_json::json!(v));
        }
        LogicalStep {
            step_id: id.into(),
            block_name: block_name.into(),
            config: cfg,
            when: None,
            input: None,
        }
    }

    fn logical_with_when(
        block_name: &str,
        id: &str,
        config: Vec<(&str, f32)>,
        when: &str,
    ) -> LogicalStep {
        let mut ls = logical(block_name, id, config);
        ls.when = Some(when.into());
        ls
    }

    fn logical_with_input(
        block_name: &str,
        id: &str,
        config: Vec<(&str, f32)>,
        input: &str,
    ) -> LogicalStep {
        let mut ls = logical(block_name, id, config);
        ls.input = Some(input.into());
        ls
    }

    use strum::EnumCount;
    const INDICATOR_COUNT: usize = engine_data::Indicator::COUNT;

    fn make_store(
        n_rows: usize,
        n_cols: usize,
        overrides: &[(Indicator, Vec<f32>)],
    ) -> DataStore {
        let mut daily: Vec<WideMatrix> = (0..INDICATOR_COUNT)
            .map(|_| WideMatrix::new(vec![f32::NAN; n_rows * n_cols], n_rows, n_cols))
            .collect();

        for (ind, data) in overrides {
            daily[*ind as usize] = WideMatrix::new(data.clone(), n_rows, n_cols);
        }

        let axes = engine_data::Axes {
            dates: vec![0; n_rows],
            tickers: (0..n_cols).map(|i| format!("T{i}")).collect(),
            ticker_idx: (0..n_cols).map(|i| (format!("T{i}"), i)).collect(),
            spy_col: None,
            etf_cols: vec![false; n_cols],
            n_rows,
            n_cols,
            trading_hours: 6.5,
        };

        DataStore::new(axes, daily, None)
    }

    // -- FusedCondition::check tests ---------------------------------------

    #[test]
    fn check_gt() {
        let c = FusedCondition { indicator: Indicator::Close, op: CmpOp::Gt, threshold: 5.0 };
        assert!(c.check(6.0));
        assert!(!c.check(5.0));
        assert!(!c.check(4.0));
        assert!(!c.check(f32::NAN));
    }

    #[test]
    fn check_ge() {
        let c = FusedCondition { indicator: Indicator::Close, op: CmpOp::Ge, threshold: 5.0 };
        assert!(c.check(5.0));
        assert!(c.check(6.0));
        assert!(!c.check(4.9));
    }

    #[test]
    fn check_lt() {
        let c = FusedCondition { indicator: Indicator::Close, op: CmpOp::Lt, threshold: 5.0 };
        assert!(c.check(4.0));
        assert!(!c.check(5.0));
        assert!(!c.check(6.0));
    }

    #[test]
    fn check_le() {
        let c = FusedCondition { indicator: Indicator::Close, op: CmpOp::Le, threshold: 5.0 };
        assert!(c.check(5.0));
        assert!(c.check(4.0));
        assert!(!c.check(5.1));
    }

    // -- FusedFilter::execute tests ----------------------------------------

    #[test]
    fn execute_two_conditions_correctness() {
        // 2 rows, 3 cols.
        // Condition 1: Close > 5.0
        // Condition 2: VolSma20 > 100.0
        let close_data = vec![10.0, 3.0, 6.0, 1.0, 8.0, 5.1];
        let vol_data = vec![200.0, 50.0, 150.0, 300.0, 90.0, 110.0];
        let store = make_store(2, 3, &[
            (Indicator::Close, close_data),
            (Indicator::VolSma20, vol_data),
        ]);

        // All-true input mask.
        let mut input = WideMask::new_false(2, 3);
        for r in 0..2 { for c in 0..3 { input.set(r, c, true); } }

        let filter = FusedFilter {
            conditions: vec![
                FusedCondition { indicator: Indicator::Close, op: CmpOp::Gt, threshold: 5.0 },
                FusedCondition { indicator: Indicator::VolSma20, op: CmpOp::Gt, threshold: 100.0 },
            ],
            step_id: "test".into(),
        };

        let result = filter.execute(&store, &input, 0..2);

        // (0,0): close=10>5 AND vol=200>100 -> true
        assert!(result.get(0, 0));
        // (0,1): close=3 not >5 -> false
        assert!(!result.get(0, 1));
        // (0,2): close=6>5 AND vol=150>100 -> true
        assert!(result.get(0, 2));
        // (1,0): close=1 not >5 -> false
        assert!(!result.get(1, 0));
        // (1,1): close=8>5 but vol=90 not >100 -> false
        assert!(!result.get(1, 1));
        // (1,2): close=5.1>5 AND vol=110>100 -> true
        assert!(result.get(1, 2));
    }

    #[test]
    fn execute_three_conditions() {
        // 1 row, 2 cols. Three conditions.
        let close = vec![10.0, 3.0];
        let vol = vec![200.0, 200.0];
        let dist = vec![0.1, 0.1];
        let store = make_store(1, 2, &[
            (Indicator::Close, close),
            (Indicator::VolSma20, vol),
            (Indicator::Dist52w, dist),
        ]);

        let mut input = WideMask::new_false(1, 2);
        input.set(0, 0, true);
        input.set(0, 1, true);

        let filter = FusedFilter {
            conditions: vec![
                FusedCondition { indicator: Indicator::Close, op: CmpOp::Gt, threshold: 5.0 },
                FusedCondition { indicator: Indicator::VolSma20, op: CmpOp::Gt, threshold: 100.0 },
                FusedCondition { indicator: Indicator::Dist52w, op: CmpOp::Le, threshold: 0.25 },
            ],
            step_id: "test".into(),
        };

        let result = filter.execute(&store, &input, 0..1);

        // (0,0): 10>5, 200>100, 0.1<=0.25 -> true
        assert!(result.get(0, 0));
        // (0,1): 3 not >5 -> false (short-circuits)
        assert!(!result.get(0, 1));
    }

    #[test]
    fn execute_respects_input_mask() {
        // Even if conditions pass, cells where input=false stay false.
        let close = vec![10.0, 10.0];
        let store = make_store(1, 2, &[(Indicator::Close, close)]);

        let mut input = WideMask::new_false(1, 2);
        input.set(0, 0, true);
        // (0,1) is false in input.

        let filter = FusedFilter {
            conditions: vec![
                FusedCondition { indicator: Indicator::Close, op: CmpOp::Gt, threshold: 5.0 },
            ],
            step_id: "test".into(),
        };

        let result = filter.execute(&store, &input, 0..1);
        assert!(result.get(0, 0));
        assert!(!result.get(0, 1));
    }

    #[test]
    fn execute_nan_fails_condition() {
        let close = vec![f32::NAN, 10.0];
        let store = make_store(1, 2, &[(Indicator::Close, close)]);

        let mut input = WideMask::new_false(1, 2);
        input.set(0, 0, true);
        input.set(0, 1, true);

        let filter = FusedFilter {
            conditions: vec![
                FusedCondition { indicator: Indicator::Close, op: CmpOp::Gt, threshold: 5.0 },
            ],
            step_id: "test".into(),
        };

        let result = filter.execute(&store, &input, 0..1);
        assert!(!result.get(0, 0)); // NaN fails
        assert!(result.get(0, 1));
    }

    #[test]
    fn execute_empty_input_empty_output() {
        let store = make_store(2, 2, &[]);
        let input = WideMask::new_false(2, 2);

        let filter = FusedFilter {
            conditions: vec![
                FusedCondition { indicator: Indicator::Close, op: CmpOp::Gt, threshold: 0.0 },
            ],
            step_id: "test".into(),
        };

        let result = filter.execute(&store, &input, 0..2);
        for r in 0..2 { for c in 0..2 { assert!(!result.get(r, c)); } }
    }

    #[test]
    fn execute_partial_range() {
        // Only scan row 1, skip row 0.
        let close = vec![10.0, 10.0, 10.0, 10.0];
        let store = make_store(2, 2, &[(Indicator::Close, close)]);

        let mut input = WideMask::new_false(2, 2);
        for r in 0..2 { for c in 0..2 { input.set(r, c, true); } }

        let filter = FusedFilter {
            conditions: vec![
                FusedCondition { indicator: Indicator::Close, op: CmpOp::Gt, threshold: 5.0 },
            ],
            step_id: "test".into(),
        };

        let result = filter.execute(&store, &input, 1..2);
        // Row 0 was not scanned, should remain false.
        assert!(!result.get(0, 0));
        assert!(!result.get(0, 1));
        // Row 1 was scanned and passes.
        assert!(result.get(1, 0));
        assert!(result.get(1, 1));
    }

    // -- fuse_steps tests --------------------------------------------------

    #[test]
    fn fuse_zero_steps() {
        let reg = fusable_registry();
        let result = fuse_steps(&[], &reg);
        assert!(result.is_empty());
    }

    #[test]
    fn fuse_single_fusable() {
        let reg = fusable_registry();
        let steps = vec![logical("price_floor", "s0", vec![("min", 5.0)])];
        let result = fuse_steps(&steps, &reg);

        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], PhysicalStep::Fused(_)));
        if let PhysicalStep::Fused(ref f) = result[0] {
            assert_eq!(f.conditions.len(), 1);
            assert_eq!(f.step_id, "s0"); // Single step keeps its original ID.
        }
    }

    #[test]
    fn fuse_five_consecutive_fusable() {
        let reg = fusable_registry();
        let steps = vec![
            logical("price_floor", "s0", vec![("min", 5.0)]),
            logical("volume_floor", "s1", vec![("min", 100000.0)]),
            logical("near_52w_high", "s2", vec![("max_dist", 0.25)]),
            logical("prior_move", "s3", vec![("min", 0.3)]),
            logical("extra", "s4", vec![("min", 10.0)]),
        ];
        let result = fuse_steps(&steps, &reg);

        assert_eq!(result.len(), 1);
        if let PhysicalStep::Fused(ref f) = result[0] {
            assert_eq!(f.conditions.len(), 5);
            assert!(f.step_id.starts_with("fused_"));
        } else {
            panic!("expected Fused");
        }
    }

    #[test]
    fn fuse_non_fusable_breaks_run() {
        let reg = fusable_registry();
        let steps = vec![
            logical("price_floor", "s0", vec![("min", 5.0)]),
            logical("consolidation", "s1", vec![]),
            logical("volume_floor", "s2", vec![("min", 100000.0)]),
        ];
        let result = fuse_steps(&steps, &reg);

        // Fused(price_floor) + Single(consolidation) + Fused(volume_floor)
        assert_eq!(result.len(), 3);
        assert!(matches!(result[0], PhysicalStep::Fused(_)));
        assert!(matches!(result[1], PhysicalStep::Single { .. }));
        assert!(matches!(result[2], PhysicalStep::Fused(_)));
    }

    #[test]
    fn fuse_when_conditional_prevents_fusion() {
        let reg = fusable_registry();
        let steps = vec![
            logical_with_when("price_floor", "s0", vec![("min", 5.0)], "true"),
            logical("volume_floor", "s1", vec![("min", 100000.0)]),
        ];
        let result = fuse_steps(&steps, &reg);

        // when-conditional -> Single, then Fused(volume_floor)
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], PhysicalStep::Single { .. }));
        assert!(matches!(result[1], PhysicalStep::Fused(_)));
    }

    #[test]
    fn fuse_explicit_input_prevents_fusion() {
        let reg = fusable_registry();
        let steps = vec![
            logical("price_floor", "s0", vec![("min", 5.0)]),
            logical_with_input("volume_floor", "s1", vec![("min", 100000.0)], "s0"),
        ];
        let result = fuse_steps(&steps, &reg);

        // price_floor -> Fused, volume_floor has explicit input -> Single
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], PhysicalStep::Fused(_)));
        assert!(matches!(result[1], PhysicalStep::Single { .. }));
    }

    #[test]
    fn fuse_missing_threshold_prevents_fusion() {
        let reg = fusable_registry();
        // price_floor expects "min" in config, but we don't provide it.
        let steps = vec![logical("price_floor", "s0", vec![])];
        let result = fuse_steps(&steps, &reg);

        // Cannot resolve threshold -> emitted as Single.
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], PhysicalStep::Single { .. }));
    }
}
