//! JSON schema types for strategy pipeline configs.
//!
//! A `StrategyPipeline` is the top-level definition loaded from a JSON file.
//! Each pipeline step references a block by type name (or imports a reusable
//! chain via "use"). Parameters prefixed with `@` are resolved from the
//! `params` block, whose bounds feed into CMA-ES evolution.

use serde::Deserialize;
use std::collections::HashMap;

/// Top-level strategy definition loaded from a JSON file.
#[derive(Clone, Debug, Deserialize)]
pub struct StrategyPipeline {
    pub name: String,
    #[serde(default = "default_direction")]
    pub direction: DirectionConfig,
    #[serde(default)]
    pub fill_mode: FillModeConfig,
    #[serde(default = "default_equity_fraction")]
    pub equity_fraction: f32,
    pub pipeline: Vec<StepDef>,
    pub stop: StopDef,
    pub exits: Vec<ExitDef>,
    #[serde(default)]
    pub params: HashMap<String, ParamValue>,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DirectionConfig {
    #[default]
    Long,
    Short,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FillModeConfig {
    #[default]
    NextDayOpen,
    SameDayOpen,
    DualTimeframe,
}

/// A single pipeline step.
#[derive(Clone, Debug, Deserialize)]
pub struct StepDef {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub block_type: Option<String>,
    #[serde(rename = "use")]
    pub use_block: Option<String>,
    pub when: Option<String>,
    pub input: Option<String>,
    #[serde(flatten)]
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StopDef {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(flatten)]
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExitDef {
    #[serde(rename = "type")]
    pub exit_type: String,
    #[serde(flatten)]
    pub config: HashMap<String, serde_json::Value>,
}

/// Parameter value with optional bounds for CMA-ES evolution.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    WithBounds {
        value: serde_json::Value,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        evolvable: bool,
    },
    Simple(serde_json::Value),
}

impl ParamValue {
    pub fn value(&self) -> &serde_json::Value {
        match self {
            Self::WithBounds { value, .. } => value,
            Self::Simple(v) => v,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        self.value().as_f64()
    }

    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|v| v as f32)
    }

    pub fn as_bool(&self) -> Option<bool> {
        self.value().as_bool()
    }
}

fn default_direction() -> DirectionConfig {
    DirectionConfig::Long
}

fn default_equity_fraction() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal_config() {
        let json = r#"{
            "name": "test_strategy",
            "pipeline": [
                { "type": "exclude_etf" }
            ],
            "stop": { "type": "atr_capped_low" },
            "exits": [{ "type": "stop_loss" }]
        }"#;
        let cfg: StrategyPipeline = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.name, "test_strategy");
        assert_eq!(cfg.pipeline.len(), 1);
        assert_eq!(cfg.equity_fraction, 1.0);
        assert!(matches!(cfg.direction, DirectionConfig::Long));
    }

    #[test]
    fn deserialize_with_bounds() {
        let json = r#"{
            "name": "bounded",
            "pipeline": [],
            "stop": { "type": "fixed_pct" },
            "exits": [],
            "params": {
                "min_price": { "value": 5.0, "min": 1.0, "max": 50.0 },
                "regime": { "value": true, "evolvable": true }
            }
        }"#;
        let cfg: StrategyPipeline = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.params.len(), 2);

        let mp = &cfg.params["min_price"];
        assert_eq!(mp.as_f64(), Some(5.0));
        if let ParamValue::WithBounds { min, max, .. } = mp {
            assert_eq!(*min, Some(1.0));
            assert_eq!(*max, Some(50.0));
        } else {
            panic!("expected WithBounds");
        }

        let regime = &cfg.params["regime"];
        assert_eq!(regime.as_bool(), Some(true));
    }

    #[test]
    fn deserialize_simple_param() {
        let json = r#"42"#;
        let p: ParamValue = serde_json::from_str(json).unwrap();
        assert_eq!(p.as_f64(), Some(42.0));
        assert!(matches!(p, ParamValue::Simple(_)));
    }

    #[test]
    fn missing_optional_fields() {
        let json = r#"{
            "name": "sparse",
            "pipeline": [{ "type": "price_floor" }],
            "stop": { "type": "fixed_pct" },
            "exits": []
        }"#;
        let cfg: StrategyPipeline = serde_json::from_str(json).unwrap();
        assert!(cfg.params.is_empty());
        assert!(matches!(cfg.fill_mode, FillModeConfig::NextDayOpen));
        let step = &cfg.pipeline[0];
        assert!(step.id.is_none());
        assert!(step.use_block.is_none());
        assert!(step.when.is_none());
        assert!(step.input.is_none());
    }

    #[test]
    fn step_with_use_block() {
        let json = r#"{
            "name": "reuse",
            "pipeline": [{ "use": "stock_universe" }],
            "stop": { "type": "fixed_pct" },
            "exits": []
        }"#;
        let cfg: StrategyPipeline = serde_json::from_str(json).unwrap();
        let step = &cfg.pipeline[0];
        assert_eq!(step.use_block.as_deref(), Some("stock_universe"));
        assert!(step.block_type.is_none());
    }
}
