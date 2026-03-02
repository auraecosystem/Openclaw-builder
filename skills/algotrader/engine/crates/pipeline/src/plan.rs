//! Plan compilation: LogicalStep -> PhysicalPlan with filter fusion.
//!
//! Transforms a parsed and validated `StrategyPipeline` into an optimized
//! `PhysicalPlan`. The filter phase (all steps before the signal-producing
//! step) is scanned for consecutive fusable blocks, which are coalesced into
//! `FusedFilter` operations for vectorized single-pass execution.

use std::collections::HashMap;

use crate::blackboard::SlotType;
use crate::config::{ExitDef, StepDef, StopDef, StrategyPipeline};
use crate::fuse::{fuse_steps, FusedFilter};
use crate::registry::BlockRegistry;

// ---------------------------------------------------------------------------
// LogicalStep: validated pipeline step with resolved config
// ---------------------------------------------------------------------------

/// A validated pipeline step ready for physical plan compilation.
///
/// Built from a `StepDef` after parsing and validation. Carries the resolved
/// block name, config map (with @params already interpolated), optional
/// when-condition, optional explicit input reference, and a unique step ID.
#[derive(Clone, Debug)]
pub struct LogicalStep {
    pub step_id: String,
    pub block_name: String,
    pub config: HashMap<String, serde_json::Value>,
    pub when: Option<String>,
    pub input: Option<String>,
}

impl LogicalStep {
    /// Build a LogicalStep from a parsed StepDef.
    ///
    /// Requires that the StepDef has been through parse_pipeline (IDs assigned,
    /// uses expanded, params interpolated).
    pub fn from_step_def(step: &StepDef) -> anyhow::Result<Self> {
        let step_id = step
            .id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("step missing ID (parse phase should assign one)"))?;
        let block_name = step
            .block_type
            .clone()
            .ok_or_else(|| anyhow::anyhow!("step '{step_id}' missing block type"))?;

        Ok(Self {
            step_id,
            block_name,
            config: step.config.clone(),
            when: step.when.clone(),
            input: step.input.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// PhysicalStep: either a single block dispatch or a fused filter
// ---------------------------------------------------------------------------

/// A physical execution step: either a single block dispatch or a fused filter.
#[derive(Clone, Debug)]
pub enum PhysicalStep {
    /// Dispatch to a single registered block.
    Single {
        block_name: String,
        config: HashMap<String, serde_json::Value>,
        step_id: String,
        input_id: Option<String>,
        when: Option<String>,
    },
    /// Multiple simple indicator-threshold filters coalesced into one pass.
    Fused(FusedFilter),
}

// ---------------------------------------------------------------------------
// PhysicalPlan: optimized execution plan
// ---------------------------------------------------------------------------

/// Optimized execution plan for a strategy pipeline.
///
/// The filter phase narrows the universe (producing WideMask). The signal step
/// generates entries/exits/stops (producing SignalSet). Stop and exit configs
/// are carried through for DynamicSetup to map into ExitRule enums.
#[derive(Clone, Debug)]
pub struct PhysicalPlan {
    pub filter_steps: Vec<PhysicalStep>,
    pub signal_step: Option<PhysicalStep>,
    pub stop: StopDef,
    pub exits: Vec<ExitDef>,
}

// ---------------------------------------------------------------------------
// compile_plan: LogicalStep vec -> PhysicalPlan
// ---------------------------------------------------------------------------

/// Compile a parsed+validated pipeline into an optimized PhysicalPlan.
///
/// Walks the pipeline steps, identifies the signal-producing step (the first
/// step whose block output_type is Signals), puts everything before it into
/// the filter phase, then applies fusion to consecutive fusable filter steps.
pub fn compile_plan(
    pipeline: &StrategyPipeline,
    registry: &BlockRegistry,
) -> anyhow::Result<PhysicalPlan> {
    // Build LogicalSteps from the parsed pipeline.
    let logical_steps: Vec<LogicalStep> = pipeline
        .pipeline
        .iter()
        .map(LogicalStep::from_step_def)
        .collect::<anyhow::Result<Vec<_>>>()?;

    // Find the signal-producing step (first step with output_type == Signals).
    let signal_idx = logical_steps.iter().position(|ls| {
        registry
            .get(&ls.block_name)
            .map(|b| b.output_type() == SlotType::Signals)
            .unwrap_or(false)
    });

    let (filter_logical, signal_logical) = match signal_idx {
        Some(idx) => (&logical_steps[..idx], Some(&logical_steps[idx])),
        None => (logical_steps.as_slice(), None),
    };

    // Apply fusion to the filter phase.
    let filter_steps = fuse_steps(filter_logical, registry);

    // Build the signal step (always a Single dispatch, never fused).
    let signal_step = signal_logical.map(|ls| PhysicalStep::Single {
        block_name: ls.block_name.clone(),
        config: ls.config.clone(),
        step_id: ls.step_id.clone(),
        input_id: ls.input.clone(),
        when: ls.when.clone(),
    });

    Ok(PhysicalPlan {
        filter_steps,
        signal_step,
        stop: pipeline.stop.clone(),
        exits: pipeline.exits.clone(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blackboard::{Slot, SlotType};
    use crate::blocks::{Block, BlockContext, CmpOp, FusableSpec};
    use crate::config::*;
    use engine_data::Indicator;
    use engine_types::WideMask;

    // -- Mock blocks -------------------------------------------------------

    /// Fusable filter block for testing.
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

    /// Non-fusable mask block.
    struct MockNonFusable(&'static str);

    impl Block for MockNonFusable {
        fn name(&self) -> &'static str { self.0 }
        fn output_type(&self) -> SlotType { SlotType::Mask }
        fn input_type(&self) -> Option<SlotType> { None }
        fn execute(&self, _ctx: &BlockContext) -> anyhow::Result<Slot> {
            Ok(Slot::Mask(WideMask::new_false(1, 1)))
        }
    }

    /// Signal-producing block.
    struct MockSignalBlock;

    impl Block for MockSignalBlock {
        fn name(&self) -> &'static str { "breakout_above" }
        fn output_type(&self) -> SlotType { SlotType::Signals }
        fn execute(&self, _ctx: &BlockContext) -> anyhow::Result<Slot> {
            unimplemented!()
        }
    }

    /// Stop block (mask output for validation to pass).
    struct MockStopBlock;

    impl Block for MockStopBlock {
        fn name(&self) -> &'static str { "fixed_pct" }
        fn output_type(&self) -> SlotType { SlotType::Matrix }
        fn input_type(&self) -> Option<SlotType> { None }
        fn execute(&self, _ctx: &BlockContext) -> anyhow::Result<Slot> {
            unimplemented!()
        }
    }

    // -- Helpers -----------------------------------------------------------

    fn test_registry() -> BlockRegistry {
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
        reg.register(Box::new(MockNonFusable("consolidation")));
        reg.register(Box::new(MockNonFusable("regime_ema")));
        reg.register(Box::new(MockSignalBlock));
        reg.register(Box::new(MockStopBlock));
        reg
    }

    fn make_step(block_type: &str, id: &str) -> StepDef {
        StepDef {
            id: Some(id.into()),
            block_type: Some(block_type.into()),
            use_block: None,
            when: None,
            input: None,
            config: HashMap::new(),
        }
    }

    fn make_step_with_config(
        block_type: &str,
        id: &str,
        config: HashMap<String, serde_json::Value>,
    ) -> StepDef {
        StepDef {
            id: Some(id.into()),
            block_type: Some(block_type.into()),
            use_block: None,
            when: None,
            input: None,
            config,
        }
    }

    fn make_pipeline(steps: Vec<StepDef>) -> StrategyPipeline {
        StrategyPipeline {
            name: "test".into(),
            direction: DirectionConfig::Long,
            fill_mode: FillModeConfig::NextDayOpen,
            equity_fraction: 1.0,
            pipeline: steps,
            stop: StopDef {
                block_type: "fixed_pct".into(),
                config: HashMap::new(),
            },
            exits: vec![ExitDef {
                exit_type: "stop_loss".into(),
                config: HashMap::new(),
            }],
            params: HashMap::new(),
        }
    }

    // -- Tests -------------------------------------------------------------

    #[test]
    fn compile_all_fusable_produces_single_fused() {
        let reg = test_registry();
        let mut cfg1 = HashMap::new();
        cfg1.insert("min".into(), serde_json::json!(5.0));
        let mut cfg2 = HashMap::new();
        cfg2.insert("min".into(), serde_json::json!(100000.0));
        let mut cfg3 = HashMap::new();
        cfg3.insert("max_dist".into(), serde_json::json!(0.25));

        let steps = vec![
            make_step_with_config("price_floor", "s0", cfg1),
            make_step_with_config("volume_floor", "s1", cfg2),
            make_step_with_config("near_52w_high", "s2", cfg3),
        ];
        let pipeline = make_pipeline(steps);
        let plan = compile_plan(&pipeline, &reg).unwrap();

        // All three fusable steps should coalesce into a single FusedFilter.
        assert_eq!(plan.filter_steps.len(), 1);
        assert!(matches!(plan.filter_steps[0], PhysicalStep::Fused(_)));
        if let PhysicalStep::Fused(ref fused) = plan.filter_steps[0] {
            assert_eq!(fused.conditions.len(), 3);
        }
        assert!(plan.signal_step.is_none());
    }

    #[test]
    fn compile_non_fusable_breaks_run() {
        let reg = test_registry();
        let mut cfg1 = HashMap::new();
        cfg1.insert("min".into(), serde_json::json!(5.0));
        let mut cfg2 = HashMap::new();
        cfg2.insert("min".into(), serde_json::json!(100000.0));

        let steps = vec![
            make_step_with_config("price_floor", "s0", cfg1.clone()),
            make_step("consolidation", "s1"),
            make_step_with_config("volume_floor", "s2", cfg2),
        ];
        let pipeline = make_pipeline(steps);
        let plan = compile_plan(&pipeline, &reg).unwrap();

        // Fused(price_floor) + Single(consolidation) + Fused(volume_floor)
        assert_eq!(plan.filter_steps.len(), 3);
        assert!(matches!(plan.filter_steps[0], PhysicalStep::Fused(_)));
        assert!(matches!(plan.filter_steps[1], PhysicalStep::Single { .. }));
        assert!(matches!(plan.filter_steps[2], PhysicalStep::Fused(_)));
    }

    #[test]
    fn compile_when_conditional_prevents_fusion() {
        let reg = test_registry();
        let mut cfg1 = HashMap::new();
        cfg1.insert("min".into(), serde_json::json!(5.0));
        let mut cfg2 = HashMap::new();
        cfg2.insert("min".into(), serde_json::json!(100000.0));

        let mut step1 = make_step_with_config("price_floor", "s0", cfg1);
        // when-conditional prevents fusion
        step1.when = Some("true".into());

        let steps = vec![step1, make_step_with_config("volume_floor", "s1", cfg2)];
        let pipeline = make_pipeline(steps);
        let plan = compile_plan(&pipeline, &reg).unwrap();

        // price_floor has when -> Single, volume_floor -> Fused
        assert_eq!(plan.filter_steps.len(), 2);
        assert!(matches!(plan.filter_steps[0], PhysicalStep::Single { .. }));
        assert!(matches!(plan.filter_steps[1], PhysicalStep::Fused(_)));
    }

    #[test]
    fn compile_signal_step_identified() {
        let reg = test_registry();
        let mut cfg1 = HashMap::new();
        cfg1.insert("min".into(), serde_json::json!(5.0));

        let steps = vec![
            make_step_with_config("price_floor", "s0", cfg1),
            make_step("breakout_above", "s1"),
        ];
        let pipeline = make_pipeline(steps);
        let plan = compile_plan(&pipeline, &reg).unwrap();

        // One filter step (fused price_floor), one signal step.
        assert_eq!(plan.filter_steps.len(), 1);
        assert!(plan.signal_step.is_some());
        if let Some(PhysicalStep::Single { ref block_name, .. }) = plan.signal_step {
            assert_eq!(block_name, "breakout_above");
        } else {
            panic!("signal_step should be Single");
        }
    }

    #[test]
    fn compile_empty_pipeline() {
        let reg = test_registry();
        let pipeline = make_pipeline(vec![]);
        let plan = compile_plan(&pipeline, &reg).unwrap();

        assert!(plan.filter_steps.is_empty());
        assert!(plan.signal_step.is_none());
    }

    #[test]
    fn compile_carries_stop_and_exits() {
        let reg = test_registry();
        let pipeline = make_pipeline(vec![]);
        let plan = compile_plan(&pipeline, &reg).unwrap();

        assert_eq!(plan.stop.block_type, "fixed_pct");
        assert_eq!(plan.exits.len(), 1);
        assert_eq!(plan.exits[0].exit_type, "stop_loss");
    }

    #[test]
    fn logical_step_from_step_def_requires_id() {
        let step = StepDef {
            id: None,
            block_type: Some("test".into()),
            use_block: None,
            when: None,
            input: None,
            config: HashMap::new(),
        };
        assert!(LogicalStep::from_step_def(&step).is_err());
    }

    #[test]
    fn logical_step_from_step_def_requires_block_type() {
        let step = StepDef {
            id: Some("s0".into()),
            block_type: None,
            use_block: None,
            when: None,
            input: None,
            config: HashMap::new(),
        };
        assert!(LogicalStep::from_step_def(&step).is_err());
    }

    #[test]
    fn compile_explicit_input_prevents_fusion() {
        let reg = test_registry();
        let mut cfg1 = HashMap::new();
        cfg1.insert("min".into(), serde_json::json!(5.0));
        let mut cfg2 = HashMap::new();
        cfg2.insert("min".into(), serde_json::json!(100000.0));

        let mut step2 = make_step_with_config("volume_floor", "s1", cfg2);
        step2.input = Some("s0".into());

        let steps = vec![make_step_with_config("price_floor", "s0", cfg1), step2];
        let pipeline = make_pipeline(steps);
        let plan = compile_plan(&pipeline, &reg).unwrap();

        // volume_floor has explicit input -> cannot fuse, breaks the run.
        // price_floor -> Fused, volume_floor -> Single
        assert_eq!(plan.filter_steps.len(), 2);
        assert!(matches!(plan.filter_steps[0], PhysicalStep::Fused(_)));
        assert!(matches!(plan.filter_steps[1], PhysicalStep::Single { .. }));
    }
}
