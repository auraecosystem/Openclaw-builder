//! Pipeline validation: block type checks, slot type chain, mutual exclusivity.
//!
//! Runs after parsing (all "use" references expanded, params interpolated, IDs
//! assigned). Catches configuration errors early so the executor never sees an
//! invalid pipeline.

use std::collections::HashSet;

use crate::blackboard::SlotType;
use crate::config::StrategyPipeline;
use crate::registry::BlockRegistry;

/// Validate a fully parsed pipeline against the block registry.
pub fn validate_pipeline(
    pipeline: &StrategyPipeline,
    registry: &BlockRegistry,
) -> anyhow::Result<()> {
    validate_block_types(pipeline, registry)?;
    validate_type_chain(pipeline, registry)?;
    validate_unique_ids(pipeline)?;
    validate_exit_types(pipeline)?;
    Ok(())
}

/// Every step must reference a block type that exists in the registry.
/// Unresolved "use" references (should have been expanded) are also caught here.
fn validate_block_types(
    pipeline: &StrategyPipeline,
    registry: &BlockRegistry,
) -> anyhow::Result<()> {
    for (i, step) in pipeline.pipeline.iter().enumerate() {
        if let Some(ref block_type) = step.block_type {
            if registry.get(block_type).is_none() {
                anyhow::bail!("step {i}: unknown block type '{block_type}'");
            }
        } else if step.use_block.is_some() {
            anyhow::bail!("step {i}: unresolved 'use' reference (should have been expanded)");
        } else {
            anyhow::bail!("step {i}: missing 'type' or 'use'");
        }
    }

    if registry.get(&pipeline.stop.block_type).is_none() {
        anyhow::bail!("stop: unknown block type '{}'", pipeline.stop.block_type);
    }

    Ok(())
}

/// Verify input/output slot type compatibility across consecutive steps.
///
/// Each block declares its expected input type and produced output type. If a
/// step has an explicit `input` reference, we check against that step's output;
/// otherwise we check against the immediately preceding step's output.
fn validate_type_chain(
    pipeline: &StrategyPipeline,
    registry: &BlockRegistry,
) -> anyhow::Result<()> {
    let mut prev_output: Option<SlotType> = None;

    for (i, step) in pipeline.pipeline.iter().enumerate() {
        let block_type = step.block_type.as_ref().unwrap(); // validated above
        let block = registry.get(block_type).unwrap();

        if let Some(required_input) = block.input_type() {
            if let Some(ref input_id) = step.input {
                // Explicit input reference -- find the referenced step's output type.
                let input_step = pipeline
                    .pipeline
                    .iter()
                    .find(|s| s.id.as_deref() == Some(input_id.as_str()))
                    .ok_or_else(|| anyhow::anyhow!("step {i}: input '{input_id}' not found"))?;
                let input_block_type = input_step.block_type.as_ref().unwrap();
                let input_block = registry.get(input_block_type).unwrap();
                let actual_output = input_block.output_type();
                if actual_output != required_input {
                    anyhow::bail!(
                        "step {i} ({block_type}): expects {required_input:?} input, \
                         but '{input_id}' produces {actual_output:?}"
                    );
                }
            } else if let Some(prev) = prev_output {
                // Implicit chaining -- check against previous step's output.
                if prev != required_input {
                    anyhow::bail!(
                        "step {i} ({block_type}): expects {required_input:?} input, \
                         but previous step produces {prev:?}"
                    );
                }
            }
            // First step with no explicit input and no predecessor: OK (block
            // handles "no input" gracefully, e.g. universe filters that start
            // from an all-true mask).
        }

        prev_output = Some(block.output_type());
    }

    Ok(())
}

/// No two steps may share the same ID.
fn validate_unique_ids(pipeline: &StrategyPipeline) -> anyhow::Result<()> {
    let mut seen = HashSet::new();
    for step in &pipeline.pipeline {
        if let Some(ref id) = step.id {
            if !seen.insert(id.as_str()) {
                anyhow::bail!("duplicate step ID: '{id}'");
            }
        }
    }
    Ok(())
}

/// Exit rule types must be from a known set.
fn validate_exit_types(pipeline: &StrategyPipeline) -> anyhow::Result<()> {
    const KNOWN: &[&str] = &[
        "profit_target",
        "sma_trail",
        "sma_cross",
        "stop_loss",
        "breakeven_upgrade",
        "sma_cover",
        "signal_bocpd_exit",
        "signal_kalman_stop",
    ];
    for (i, exit) in pipeline.exits.iter().enumerate() {
        if !KNOWN.contains(&exit.exit_type.as_str()) {
            anyhow::bail!("exit {i}: unknown type '{}'", exit.exit_type);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blackboard::Slot;
    use crate::blocks::{Block, BlockContext};
    use crate::config::*;
    use engine_types::WideMask;
    use std::collections::HashMap;

    // -- Mock blocks for testing -----------------------------------------

    struct MockMaskBlock {
        type_name: &'static str,
        needs_input: bool,
    }

    impl Block for MockMaskBlock {
        fn name(&self) -> &'static str {
            self.type_name
        }
        fn output_type(&self) -> SlotType {
            SlotType::Mask
        }
        fn input_type(&self) -> Option<SlotType> {
            if self.needs_input {
                Some(SlotType::Mask)
            } else {
                None
            }
        }
        fn execute(&self, _ctx: &BlockContext) -> anyhow::Result<Slot> {
            Ok(Slot::Mask(WideMask::new_false(1, 1)))
        }
    }

    struct MockMatrixBlock;

    impl Block for MockMatrixBlock {
        fn name(&self) -> &'static str {
            "matrix_producer"
        }
        fn output_type(&self) -> SlotType {
            SlotType::Matrix
        }
        fn input_type(&self) -> Option<SlotType> {
            None
        }
        fn execute(&self, _ctx: &BlockContext) -> anyhow::Result<Slot> {
            unimplemented!("test mock")
        }
    }

    // -- Helpers ----------------------------------------------------------

    fn test_registry() -> BlockRegistry {
        let mut reg = BlockRegistry::new();
        reg.register(Box::new(MockMaskBlock {
            type_name: "exclude_etf",
            needs_input: false,
        }));
        reg.register(Box::new(MockMaskBlock {
            type_name: "price_floor",
            needs_input: true,
        }));
        reg.register(Box::new(MockMaskBlock {
            type_name: "volume_floor",
            needs_input: true,
        }));
        reg.register(Box::new(MockMaskBlock {
            type_name: "breakout_above",
            needs_input: true,
        }));
        reg.register(Box::new(MockMatrixBlock));
        // Stop block
        reg.register(Box::new(MockMaskBlock {
            type_name: "fixed_pct",
            needs_input: false,
        }));
        reg
    }

    fn step(block_type: &str) -> StepDef {
        StepDef {
            id: None,
            block_type: Some(block_type.into()),
            use_block: None,
            when: None,
            input: None,
            config: HashMap::new(),
        }
    }

    fn minimal_pipeline(steps: Vec<StepDef>) -> StrategyPipeline {
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

    // -- Tests ------------------------------------------------------------

    #[test]
    fn valid_pipeline_passes() {
        let reg = test_registry();
        let mut steps = vec![step("exclude_etf"), step("price_floor")];
        // Assign IDs so uniqueness check works
        steps[0].id = Some("step_0".into());
        steps[1].id = Some("step_1".into());
        let pipeline = minimal_pipeline(steps);

        validate_pipeline(&pipeline, &reg).unwrap();
    }

    #[test]
    fn unknown_block_type_errors() {
        let reg = test_registry();
        let mut steps = vec![step("nonexistent_block")];
        steps[0].id = Some("step_0".into());
        let pipeline = minimal_pipeline(steps);

        let err = validate_pipeline(&pipeline, &reg).unwrap_err();
        assert!(err.to_string().contains("unknown block type 'nonexistent_block'"));
    }

    #[test]
    fn unknown_stop_type_errors() {
        let reg = test_registry();
        let mut pipeline = minimal_pipeline(vec![]);
        pipeline.stop.block_type = "unknown_stop".into();

        let err = validate_pipeline(&pipeline, &reg).unwrap_err();
        assert!(err.to_string().contains("unknown block type 'unknown_stop'"));
    }

    #[test]
    fn duplicate_step_id_errors() {
        let reg = test_registry();
        let mut steps = vec![step("exclude_etf"), step("price_floor")];
        steps[0].id = Some("same_id".into());
        steps[1].id = Some("same_id".into());
        let pipeline = minimal_pipeline(steps);

        let err = validate_pipeline(&pipeline, &reg).unwrap_err();
        assert!(err.to_string().contains("duplicate step ID: 'same_id'"));
    }

    #[test]
    fn unknown_exit_type_errors() {
        let reg = test_registry();
        let mut pipeline = minimal_pipeline(vec![]);
        pipeline.exits = vec![ExitDef {
            exit_type: "invented_exit".into(),
            config: HashMap::new(),
        }];

        let err = validate_pipeline(&pipeline, &reg).unwrap_err();
        assert!(err.to_string().contains("unknown type 'invented_exit'"));
    }

    #[test]
    fn type_mismatch_errors() {
        let reg = test_registry();
        // matrix_producer outputs Matrix, but price_floor expects Mask input
        let mut steps = vec![step("matrix_producer"), step("price_floor")];
        steps[0].id = Some("step_0".into());
        steps[1].id = Some("step_1".into());
        let pipeline = minimal_pipeline(steps);

        let err = validate_pipeline(&pipeline, &reg).unwrap_err();
        assert!(err.to_string().contains("expects Mask input"));
        assert!(err.to_string().contains("produces Matrix"));
    }

    #[test]
    fn explicit_input_ref_type_mismatch() {
        let reg = test_registry();
        let mut steps = vec![step("exclude_etf"), step("matrix_producer"), step("price_floor")];
        steps[0].id = Some("mask_step".into());
        steps[1].id = Some("matrix_step".into());
        steps[2].id = Some("floor_step".into());
        // price_floor explicitly references matrix_step, which produces Matrix (wrong)
        steps[2].input = Some("matrix_step".into());
        let pipeline = minimal_pipeline(steps);

        let err = validate_pipeline(&pipeline, &reg).unwrap_err();
        assert!(err.to_string().contains("expects Mask input"));
    }

    #[test]
    fn explicit_input_ref_not_found() {
        let reg = test_registry();
        let mut steps = vec![step("price_floor")];
        steps[0].id = Some("step_0".into());
        steps[0].input = Some("ghost".into());
        let pipeline = minimal_pipeline(steps);

        let err = validate_pipeline(&pipeline, &reg).unwrap_err();
        assert!(err.to_string().contains("input 'ghost' not found"));
    }

    #[test]
    fn unresolved_use_reference_errors() {
        let reg = test_registry();
        let steps = vec![StepDef {
            id: Some("step_0".into()),
            block_type: None,
            use_block: Some("should_be_expanded".into()),
            when: None,
            input: None,
            config: HashMap::new(),
        }];
        let pipeline = minimal_pipeline(steps);

        let err = validate_pipeline(&pipeline, &reg).unwrap_err();
        assert!(err.to_string().contains("unresolved 'use' reference"));
    }

    #[test]
    fn first_step_no_input_ok() {
        // A block that requires input but is the first step -- allowed because
        // it handles "no prior output" gracefully at runtime.
        let reg = test_registry();
        let mut steps = vec![step("price_floor")];
        steps[0].id = Some("step_0".into());
        let pipeline = minimal_pipeline(steps);

        validate_pipeline(&pipeline, &reg).unwrap();
    }

    #[test]
    fn all_known_exit_types_pass() {
        let reg = test_registry();
        let exits = [
            "profit_target",
            "sma_trail",
            "sma_cross",
            "stop_loss",
            "breakeven_upgrade",
            "sma_cover",
            "signal_bocpd_exit",
            "signal_kalman_stop",
        ];
        for exit_type in exits {
            let mut pipeline = minimal_pipeline(vec![]);
            pipeline.exits = vec![ExitDef {
                exit_type: exit_type.into(),
                config: HashMap::new(),
            }];
            validate_pipeline(&pipeline, &reg).unwrap();
        }
    }
}
