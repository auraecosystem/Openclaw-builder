//! Config parsing: @param interpolation, "use" block expansion, step ID assignment.
//!
//! The parse phase transforms a raw deserialized `StrategyPipeline` into a
//! fully resolved form: block library references are inlined, @-prefixed param
//! references are replaced with concrete values, and every step gets an ID.
//! This runs before validation so the validator sees a flat, resolved pipeline.

use std::collections::{HashMap, HashSet};

use crate::config::{ParamValue, StepDef, StrategyPipeline};

/// Block library: maps reusable chain names to arrays of step definitions.
pub type BlockLibrary = HashMap<String, Vec<StepDef>>;

/// Parse a strategy pipeline: expand "use" references, assign IDs, interpolate params.
///
/// Order matters: expansion must happen before interpolation so that inlined
/// steps also get their @-params resolved.
pub fn parse_pipeline(
    pipeline: &mut StrategyPipeline,
    library: &BlockLibrary,
) -> anyhow::Result<()> {
    expand_uses(pipeline, library)?;
    assign_step_ids(pipeline);
    interpolate_params(pipeline)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// @param interpolation
// ---------------------------------------------------------------------------

/// Resolve all @param references in a pipeline's step configs, stop, and exits.
pub fn interpolate_params(pipeline: &mut StrategyPipeline) -> anyhow::Result<()> {
    for step in &mut pipeline.pipeline {
        interpolate_map(&mut step.config, &pipeline.params)?;

        // Validate "when" param references (must point to a boolean param).
        if let Some(ref when) = step.when {
            if let Some(stripped) = when.strip_prefix('@') {
                let param = pipeline
                    .params
                    .get(stripped)
                    .ok_or_else(|| anyhow::anyhow!("param not found: @{stripped}"))?;
                if param.as_bool().is_none() {
                    anyhow::bail!("'when' param @{stripped} must be a boolean");
                }
            }
        }
    }

    interpolate_map(&mut pipeline.stop.config, &pipeline.params)?;

    for exit in &mut pipeline.exits {
        interpolate_map(&mut exit.config, &pipeline.params)?;
    }

    Ok(())
}

fn interpolate_map(
    config: &mut HashMap<String, serde_json::Value>,
    params: &HashMap<String, ParamValue>,
) -> anyhow::Result<()> {
    for (_key, value) in config.iter_mut() {
        interpolate_value(value, params)?;
    }
    Ok(())
}

/// Recursively walk a JSON value, replacing any string starting with `@` with
/// the corresponding param's concrete value.
fn interpolate_value(
    value: &mut serde_json::Value,
    params: &HashMap<String, ParamValue>,
) -> anyhow::Result<()> {
    match value {
        serde_json::Value::String(s) => {
            if let Some(param_name) = s.strip_prefix('@') {
                let param = params
                    .get(param_name)
                    .ok_or_else(|| anyhow::anyhow!("param not found: @{param_name}"))?;
                *value = param.value().clone();
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                interpolate_value(item, params)?;
            }
        }
        serde_json::Value::Object(map) => {
            for (_k, v) in map.iter_mut() {
                interpolate_value(v, params)?;
            }
        }
        _ => {} // numbers, bools, null -- no interpolation needed
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// "use" block expansion
// ---------------------------------------------------------------------------

/// Load a block library from a JSON file.
///
/// Expected format: `{ "chain_name": [ { step }, ... ], ... }`
pub fn load_block_library(path: &std::path::Path) -> anyhow::Result<BlockLibrary> {
    let content = std::fs::read_to_string(path)?;
    let raw: HashMap<String, Vec<serde_json::Value>> = serde_json::from_str(&content)?;
    let mut result = HashMap::new();
    for (name, steps_json) in raw {
        let steps: Vec<StepDef> = steps_json
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("block library '{name}': {e}"))?;
        result.insert(name, steps);
    }
    Ok(result)
}

/// Expand all "use" references in a pipeline, inlining referenced block chains.
/// Detects circular references.
pub fn expand_uses(
    pipeline: &mut StrategyPipeline,
    library: &BlockLibrary,
) -> anyhow::Result<()> {
    let mut expanded = Vec::new();
    let mut seen = HashSet::new();
    expand_steps(&pipeline.pipeline, library, &mut expanded, &mut seen)?;
    pipeline.pipeline = expanded;
    Ok(())
}

fn expand_steps(
    steps: &[StepDef],
    library: &BlockLibrary,
    out: &mut Vec<StepDef>,
    seen: &mut HashSet<String>,
) -> anyhow::Result<()> {
    for step in steps {
        if let Some(ref use_name) = step.use_block {
            if !seen.insert(use_name.clone()) {
                anyhow::bail!("circular block reference: '{use_name}'");
            }
            let chain = library
                .get(use_name)
                .ok_or_else(|| anyhow::anyhow!("unknown block library entry: '{use_name}'"))?;
            expand_steps(chain, library, out, seen)?;
            seen.remove(use_name);
        } else {
            out.push(step.clone());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Step ID assignment
// ---------------------------------------------------------------------------

/// Assign auto-generated IDs (`step_0`, `step_1`, ...) to steps that lack one.
pub fn assign_step_ids(pipeline: &mut StrategyPipeline) {
    for (i, step) in pipeline.pipeline.iter_mut().enumerate() {
        if step.id.is_none() {
            step.id = Some(format!("step_{i}"));
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use serde_json::json;

    /// Build a minimal pipeline with given steps and params.
    fn make_pipeline(
        steps: Vec<StepDef>,
        params: HashMap<String, ParamValue>,
    ) -> StrategyPipeline {
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
            exits: vec![],
            params,
        }
    }

    fn step_with_type(block_type: &str) -> StepDef {
        StepDef {
            id: None,
            block_type: Some(block_type.into()),
            use_block: None,
            when: None,
            input: None,
            config: HashMap::new(),
        }
    }

    fn step_with_config(block_type: &str, config: HashMap<String, serde_json::Value>) -> StepDef {
        StepDef {
            id: None,
            block_type: Some(block_type.into()),
            use_block: None,
            when: None,
            input: None,
            config,
        }
    }

    fn use_step(name: &str) -> StepDef {
        StepDef {
            id: None,
            block_type: None,
            use_block: Some(name.into()),
            when: None,
            input: None,
            config: HashMap::new(),
        }
    }

    #[test]
    fn interpolate_simple_param() {
        let mut params = HashMap::new();
        params.insert(
            "min_price".into(),
            ParamValue::WithBounds {
                value: json!(5.0),
                min: Some(1.0),
                max: Some(50.0),
                evolvable: false,
            },
        );

        let mut config = HashMap::new();
        config.insert("value".into(), json!("@min_price"));

        let steps = vec![step_with_config("price_floor", config)];
        let mut pipeline = make_pipeline(steps, params);

        interpolate_params(&mut pipeline).unwrap();
        assert_eq!(pipeline.pipeline[0].config["value"], json!(5.0));
    }

    #[test]
    fn interpolate_missing_param_errors() {
        let mut config = HashMap::new();
        config.insert("value".into(), json!("@unknown"));

        let steps = vec![step_with_config("price_floor", config)];
        let mut pipeline = make_pipeline(steps, HashMap::new());

        let err = interpolate_params(&mut pipeline).unwrap_err();
        assert!(err.to_string().contains("param not found: @unknown"));
    }

    #[test]
    fn interpolate_nested_array() {
        let mut params = HashMap::new();
        params.insert("a".into(), ParamValue::Simple(json!(10)));
        params.insert("b".into(), ParamValue::Simple(json!(20)));

        let mut config = HashMap::new();
        config.insert("values".into(), json!(["@a", "@b"]));

        let steps = vec![step_with_config("test_block", config)];
        let mut pipeline = make_pipeline(steps, params);

        interpolate_params(&mut pipeline).unwrap();
        assert_eq!(pipeline.pipeline[0].config["values"], json!([10, 20]));
    }

    #[test]
    fn expand_use_inlines_steps() {
        let mut library = BlockLibrary::new();
        library.insert(
            "stock_universe".into(),
            vec![
                step_with_type("exclude_etf"),
                step_with_type("price_floor"),
                step_with_type("volume_floor"),
            ],
        );

        let steps = vec![use_step("stock_universe"), step_with_type("breakout_above")];
        let mut pipeline = make_pipeline(steps, HashMap::new());

        expand_uses(&mut pipeline, &library).unwrap();
        assert_eq!(pipeline.pipeline.len(), 4);
        assert_eq!(
            pipeline.pipeline[0].block_type.as_deref(),
            Some("exclude_etf")
        );
        assert_eq!(
            pipeline.pipeline[3].block_type.as_deref(),
            Some("breakout_above")
        );
    }

    #[test]
    fn expand_circular_use_errors() {
        // "a" uses "b", "b" uses "a"
        let mut library = BlockLibrary::new();
        library.insert("a".into(), vec![use_step("b")]);
        library.insert("b".into(), vec![use_step("a")]);

        let steps = vec![use_step("a")];
        let mut pipeline = make_pipeline(steps, HashMap::new());

        let err = expand_uses(&mut pipeline, &library).unwrap_err();
        assert!(err.to_string().contains("circular block reference"));
    }

    #[test]
    fn expand_unknown_use_errors() {
        let library = BlockLibrary::new();
        let steps = vec![use_step("nonexistent")];
        let mut pipeline = make_pipeline(steps, HashMap::new());

        let err = expand_uses(&mut pipeline, &library).unwrap_err();
        assert!(err.to_string().contains("unknown block library entry"));
    }

    #[test]
    fn assign_ids_fills_gaps() {
        let steps = vec![
            {
                let mut s = step_with_type("a");
                s.id = Some("keep_me".into());
                s
            },
            step_with_type("b"),
            step_with_type("c"),
        ];
        let mut pipeline = make_pipeline(steps, HashMap::new());

        assign_step_ids(&mut pipeline);
        assert_eq!(pipeline.pipeline[0].id.as_deref(), Some("keep_me"));
        assert_eq!(pipeline.pipeline[1].id.as_deref(), Some("step_1"));
        assert_eq!(pipeline.pipeline[2].id.as_deref(), Some("step_2"));
    }

    #[test]
    fn when_must_be_bool_param() {
        let mut params = HashMap::new();
        params.insert("not_a_bool".into(), ParamValue::Simple(json!(42.0)));

        let mut step = step_with_type("regime_ema");
        step.when = Some("@not_a_bool".into());

        let mut pipeline = make_pipeline(vec![step], params);

        let err = interpolate_params(&mut pipeline).unwrap_err();
        assert!(err.to_string().contains("must be a boolean"));
    }

    #[test]
    fn when_with_valid_bool_param() {
        let mut params = HashMap::new();
        params.insert("regime".into(), ParamValue::Simple(json!(true)));

        let mut step = step_with_type("regime_ema");
        step.when = Some("@regime".into());

        let mut pipeline = make_pipeline(vec![step], params);
        interpolate_params(&mut pipeline).unwrap();
    }

    #[test]
    fn interpolate_stop_and_exit_configs() {
        let mut params = HashMap::new();
        params.insert("stop_pct".into(), ParamValue::Simple(json!(0.05)));
        params.insert("min_bars".into(), ParamValue::Simple(json!(5)));

        let mut stop_config = HashMap::new();
        stop_config.insert("fallback_pct".into(), json!("@stop_pct"));

        let mut exit_config = HashMap::new();
        exit_config.insert("min_bars".into(), json!("@min_bars"));

        let mut pipeline = make_pipeline(vec![], params);
        pipeline.stop.config = stop_config;
        pipeline.exits = vec![ExitDef {
            exit_type: "profit_target".into(),
            config: exit_config,
        }];

        interpolate_params(&mut pipeline).unwrap();
        assert_eq!(pipeline.stop.config["fallback_pct"], json!(0.05));
        assert_eq!(pipeline.exits[0].config["min_bars"], json!(5));
    }

    #[test]
    fn parse_pipeline_runs_all_phases() {
        let mut params = HashMap::new();
        params.insert("val".into(), ParamValue::Simple(json!(99)));

        let mut library = BlockLibrary::new();
        library.insert("chain".into(), vec![step_with_type("a")]);

        let mut config = HashMap::new();
        config.insert("x".into(), json!("@val"));

        let steps = vec![use_step("chain"), step_with_config("b", config)];
        let mut pipeline = make_pipeline(steps, params);

        parse_pipeline(&mut pipeline, &library).unwrap();

        // "use" expanded: chain -> "a", then "b"
        assert_eq!(pipeline.pipeline.len(), 2);
        assert_eq!(pipeline.pipeline[0].block_type.as_deref(), Some("a"));
        // IDs assigned
        assert_eq!(pipeline.pipeline[0].id.as_deref(), Some("step_0"));
        assert_eq!(pipeline.pipeline[1].id.as_deref(), Some("step_1"));
        // Params interpolated
        assert_eq!(pipeline.pipeline[1].config["x"], json!(99));
    }
}
