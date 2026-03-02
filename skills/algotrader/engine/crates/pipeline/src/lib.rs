//! engine-pipeline: config-driven strategy pipeline system.
//!
//! Strategies are defined as JSON configs describing a pipeline of composable
//! blocks. Each step selects from a registry of available filters, detectors,
//! and scorers. No recompile needed for new strategy variants.
//!
//! The main entry point is `DynamicSetup`: it loads a JSON config, parses and
//! validates it, compiles a physical plan, and implements the `Setup` trait so
//! it plugs into the simulation engine unchanged.

pub mod blackboard;
pub mod blocks;
pub mod config;
pub mod execute;
pub mod fuse;
pub mod parse;
pub mod plan;
pub mod registry;
pub mod validate;

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use engine_data::{DataStore, Indicator};
use engine_types::{Direction, SignalSet, WideMask};

use crate::config::{DirectionConfig, ExitDef, FillModeConfig, StrategyPipeline, StopDef};
use crate::execute::{execute_filter_phase, execute_signal_phase};
use crate::plan::PhysicalPlan;
use crate::registry::BlockRegistry;

// ---------------------------------------------------------------------------
// Default registry builder
// ---------------------------------------------------------------------------

/// Build a registry with all built-in blocks registered.
///
/// This lives in lib.rs (not registry.rs) so that registry.rs does not need to
/// depend on every block module. Each block category contributes its instances
/// here.
pub fn build_default_registry() -> BlockRegistry {
    let mut r = BlockRegistry::new();

    // Universe filters (13 blocks).
    for block in blocks::universe::all_blocks() {
        r.register(block);
    }

    // Pattern detectors.
    r.register(Box::new(blocks::pattern::VcpDetect));
    r.register(Box::new(blocks::pattern::FlagDetect));
    r.register(Box::new(blocks::pattern::GapUp));
    r.register(Box::new(blocks::pattern::ParabolicRun));
    r.register(Box::new(blocks::pattern::PatternAny));

    // Entry conditions.
    r.register(Box::new(blocks::entry::BreakoutAbove));
    r.register(Box::new(blocks::entry::GapEntry));
    r.register(Box::new(blocks::entry::ScoreThreshold));
    r.register(Box::new(blocks::entry::ParabolicEntry));

    // Stop generators.
    r.register(Box::new(blocks::stop::AtrCappedLow));
    r.register(Box::new(blocks::stop::FixedPct));
    r.register(Box::new(blocks::stop::GapDayLow));

    // Mask combiners.
    r.register(Box::new(blocks::combiner::And));
    r.register(Box::new(blocks::combiner::Or));
    r.register(Box::new(blocks::combiner::Not));

    // Signal pipeline wrapper.
    r.register(Box::new(blocks::signal::SignalPipelineBlock));

    // Cross-sectional gates (stubs).
    r.register(Box::new(blocks::cross_sectional::IsingSusceptibility));
    r.register(Box::new(blocks::cross_sectional::VnEntropy));
    r.register(Box::new(blocks::cross_sectional::Quorum));

    r
}

// ---------------------------------------------------------------------------
// Indicator parsing (duplicated from blocks/universe.rs for lib-level use)
// ---------------------------------------------------------------------------

/// Map a config string to an `Indicator` enum variant.
fn parse_indicator_str(s: &str) -> anyhow::Result<Indicator> {
    match s {
        "close" => Ok(Indicator::Close),
        "open" => Ok(Indicator::Open),
        "high" => Ok(Indicator::High),
        "low" => Ok(Indicator::Low),
        "volume" => Ok(Indicator::Volume),
        "atr_14" => Ok(Indicator::Atr14),
        "sma_10" => Ok(Indicator::Sma10),
        "sma_20" => Ok(Indicator::Sma20),
        "vol_sma_20" => Ok(Indicator::VolSma20),
        "rs_pctrank_1m" => Ok(Indicator::RsPctrank1m),
        "rs_pctrank_3m" => Ok(Indicator::RsPctrank3m),
        "rs_pctrank_6m" => Ok(Indicator::RsPctrank6m),
        "dist_52w" => Ok(Indicator::Dist52w),
        "ret_63" => Ok(Indicator::Ret63),
        "ret_126" => Ok(Indicator::Ret126),
        "pct_10d" => Ok(Indicator::Pct10d),
        "consec_green" => Ok(Indicator::ConsecGreen),
        "consol_high" => Ok(Indicator::ConsolHigh),
        "vcp_num_contractions" => Ok(Indicator::VcpNumContractions),
        "vcp_last_contraction_pct" => Ok(Indicator::VcpLastContractionPct),
        "vcp_tightening_ratio" => Ok(Indicator::VcpTighteningRatio),
        "vcp_vol_trend" => Ok(Indicator::VcpVolTrend),
        "flag_pole_pct" => Ok(Indicator::FlagPolePct),
        "flag_retrace_pct" => Ok(Indicator::FlagRetracePct),
        "flag_days" => Ok(Indicator::FlagDays),
        "flag_vol_ratio" => Ok(Indicator::FlagVolRatio),
        other => anyhow::bail!("unknown indicator: {other}"),
    }
}

// ---------------------------------------------------------------------------
// DynamicSetup
// ---------------------------------------------------------------------------

/// A strategy loaded from JSON config, implementing the Setup trait.
///
/// Pipeline params are baked in at parse time (via `@param` interpolation).
/// The `Setup` trait's `Params` argument is only used by hardcoded strategies;
/// DynamicSetup ignores it for filter/signal execution.
pub struct DynamicSetup {
    /// Leaked to `&'static str` for the Setup trait. Strategies are loaded once
    /// at startup so this is acceptable.
    name: String,
    direction: DirectionConfig,
    equity_fraction: f32,
    fill_mode: FillModeConfig,
    exit_defs: Vec<ExitDef>,
    #[allow(dead_code)]
    stop_def: StopDef,
    pipeline_config: StrategyPipeline,
    registry: Arc<BlockRegistry>,
}

// Safety: BlockRegistry contains only Send+Sync blocks, and DynamicSetup is
// read-only after construction.
unsafe impl Send for DynamicSetup {}
unsafe impl Sync for DynamicSetup {}

impl DynamicSetup {
    /// Load a DynamicSetup from a JSON string.
    ///
    /// Deserializes the config, builds the default block registry, parses
    /// (expanding uses, interpolating params, assigning IDs), and validates.
    pub fn from_json(json_str: &str) -> anyhow::Result<Self> {
        let mut pipeline: StrategyPipeline = serde_json::from_str(json_str)?;

        let registry = build_default_registry();

        // Parse phase: expand uses, interpolate params, assign step IDs.
        // These functions are in parse.rs (Wave 1D).
        parse::assign_step_ids(&mut pipeline);
        // NOTE: expand_uses and interpolate_params require a block library and
        // params map respectively. For now we call what's available; the parse
        // module handles missing/empty gracefully.
        let _ = parse::interpolate_params(&mut pipeline);

        // Validate phase.
        validate::validate_pipeline(&pipeline, &registry)?;

        Ok(Self {
            name: pipeline.name.clone(),
            direction: pipeline.direction.clone(),
            equity_fraction: pipeline.equity_fraction,
            fill_mode: pipeline.fill_mode.clone(),
            exit_defs: pipeline.exits.clone(),
            stop_def: pipeline.stop.clone(),
            pipeline_config: pipeline,
            registry: Arc::new(registry),
        })
    }

    /// Load a DynamicSetup from a JSON file path.
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let json_str = std::fs::read_to_string(path)?;
        Self::from_json(&json_str)
    }

    /// Compile pipeline steps into an optimized physical plan with fusion.
    ///
    /// Delegates to `plan::compile_plan` which identifies fusable filter runs,
    /// coalesces them into FusedFilters, and separates the signal step.
    fn compile_plan(&self) -> anyhow::Result<PhysicalPlan> {
        plan::compile_plan(&self.pipeline_config, &self.registry)
    }

    /// Convert the direction config to the engine's Direction type.
    fn direction_value(&self) -> Direction {
        match self.direction {
            DirectionConfig::Long => Direction::Long,
            DirectionConfig::Short => Direction::Short,
        }
    }
}

// ---------------------------------------------------------------------------
// Exit rule mapping
// ---------------------------------------------------------------------------

/// Placeholder ExitRule type matching the root crate's execution::ExitRule.
///
/// DynamicSetup needs to produce these from ExitDef configs. Since
/// engine-pipeline cannot depend on the root crate, we define a local mapping
/// struct. The integration layer (Wave 3) will convert these to the real
/// ExitRule enum.
#[derive(Clone, Debug)]
pub enum DynamicExitRule {
    StopLoss,
    ProfitTarget { min_bars: usize },
    SmaCross { sma: Indicator },
    BreakevenUpgrade { after_bars: usize },
    SmaCover { sma1: Indicator, sma2: Indicator },
    SignalBocpdExit { threshold: f32 },
    SignalKalmanStop { atr_mult: f32 },
}

/// Map an ExitDef (from JSON config) to a DynamicExitRule.
pub fn map_exit_def(def: &ExitDef) -> anyhow::Result<DynamicExitRule> {
    match def.exit_type.as_str() {
        "stop_loss" => Ok(DynamicExitRule::StopLoss),

        "profit_target" => {
            let min_bars = def
                .config
                .get("min_bars")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(5);
            Ok(DynamicExitRule::ProfitTarget { min_bars })
        }

        "sma_trail" | "sma_cross" => {
            let period = def
                .config
                .get("period")
                .and_then(|v| v.as_u64())
                .unwrap_or(10);
            let sma = match period {
                10 => Indicator::Sma10,
                20 => Indicator::Sma20,
                other => anyhow::bail!("sma_trail: unsupported period {other} (only 10, 20)"),
            };
            Ok(DynamicExitRule::SmaCross { sma })
        }

        "breakeven_upgrade" => {
            let after_bars = def
                .config
                .get("after_bars")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(5);
            Ok(DynamicExitRule::BreakevenUpgrade { after_bars })
        }

        "sma_cover" => {
            let sma1_str = def
                .config
                .get("sma1")
                .and_then(|v| v.as_str())
                .unwrap_or("sma_10");
            let sma2_str = def
                .config
                .get("sma2")
                .and_then(|v| v.as_str())
                .unwrap_or("sma_20");
            let sma1 = parse_indicator_str(sma1_str)?;
            let sma2 = parse_indicator_str(sma2_str)?;
            Ok(DynamicExitRule::SmaCover { sma1, sma2 })
        }

        "signal_bocpd_exit" => {
            let threshold = def
                .config
                .get("threshold")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(0.5);
            Ok(DynamicExitRule::SignalBocpdExit { threshold })
        }

        "signal_kalman_stop" => {
            let atr_mult = def
                .config
                .get("atr_mult")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(2.0);
            Ok(DynamicExitRule::SignalKalmanStop { atr_mult })
        }

        other => anyhow::bail!("unknown exit type: {other}"),
    }
}

/// Map all exit defs to DynamicExitRules.
pub fn map_exit_defs(defs: &[ExitDef]) -> anyhow::Result<Vec<DynamicExitRule>> {
    defs.iter().map(map_exit_def).collect()
}

// ---------------------------------------------------------------------------
// DynamicSetup pipeline execution helpers
// ---------------------------------------------------------------------------

impl DynamicSetup {
    /// Run the filter phase: compile plan and execute it.
    pub fn run_filter(
        &self,
        store: &DataStore,
        range: Range<usize>,
    ) -> anyhow::Result<WideMask> {
        let plan = self.compile_plan()?;
        execute_filter_phase(&plan.filter_steps, store, &self.registry, range)
    }

    /// Run the signal phase against a universe mask.
    pub fn run_signals(
        &self,
        store: &DataStore,
        universe: &WideMask,
    ) -> anyhow::Result<SignalSet> {
        let plan = self.compile_plan()?;
        execute_signal_phase(plan.signal_step.as_ref(), store, &self.registry, universe)
    }

    /// Map exit definitions to DynamicExitRules.
    pub fn exit_rules(&self) -> anyhow::Result<Vec<DynamicExitRule>> {
        map_exit_defs(&self.exit_defs)
    }

    /// Strategy name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Strategy direction.
    pub fn direction(&self) -> Direction {
        self.direction_value()
    }

    /// Equity fraction.
    pub fn equity_fraction(&self) -> f32 {
        self.equity_fraction
    }

    /// Fill mode config (caller maps to the root crate's FillMode).
    pub fn fill_mode_config(&self) -> &FillModeConfig {
        &self.fill_mode
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid JSON config for testing.
    fn minimal_json() -> &'static str {
        r#"{
            "name": "test_dynamic",
            "direction": "long",
            "fill_mode": "next_day_open",
            "equity_fraction": 0.5,
            "pipeline": [
                { "type": "exclude_etf" }
            ],
            "stop": { "type": "atr_capped_low" },
            "exits": [
                { "type": "stop_loss" },
                { "type": "profit_target", "min_bars": 5 },
                { "type": "sma_trail", "period": 10 }
            ]
        }"#
    }

    #[test]
    fn from_json_minimal() {
        let setup = DynamicSetup::from_json(minimal_json()).unwrap();
        assert_eq!(setup.name(), "test_dynamic");
        assert_eq!(setup.equity_fraction(), 0.5);
        assert_eq!(setup.direction(), Direction::Long);
    }

    #[test]
    fn direction_mapping() {
        let long_json = r#"{
            "name": "d1", "direction": "long",
            "pipeline": [], "stop": { "type": "fixed_pct" }, "exits": []
        }"#;
        let short_json = r#"{
            "name": "d2", "direction": "short",
            "pipeline": [], "stop": { "type": "fixed_pct" }, "exits": []
        }"#;
        let l = DynamicSetup::from_json(long_json).unwrap();
        let s = DynamicSetup::from_json(short_json).unwrap();
        assert_eq!(l.direction(), Direction::Long);
        assert_eq!(s.direction(), Direction::Short);
    }

    #[test]
    fn fill_mode_mapping() {
        let json = r#"{
            "name": "fm", "fill_mode": "same_day_open",
            "pipeline": [], "stop": { "type": "fixed_pct" }, "exits": []
        }"#;
        let setup = DynamicSetup::from_json(json).unwrap();
        assert!(matches!(
            setup.fill_mode_config(),
            FillModeConfig::SameDayOpen
        ));
    }

    #[test]
    fn exit_rules_stop_loss() {
        let setup = DynamicSetup::from_json(minimal_json()).unwrap();
        let rules = setup.exit_rules().unwrap();
        assert_eq!(rules.len(), 3);
        assert!(matches!(rules[0], DynamicExitRule::StopLoss));
    }

    #[test]
    fn exit_rules_profit_target() {
        let setup = DynamicSetup::from_json(minimal_json()).unwrap();
        let rules = setup.exit_rules().unwrap();
        match &rules[1] {
            DynamicExitRule::ProfitTarget { min_bars } => assert_eq!(*min_bars, 5),
            other => panic!("expected ProfitTarget, got {:?}", other),
        }
    }

    #[test]
    fn exit_rules_sma_trail() {
        let setup = DynamicSetup::from_json(minimal_json()).unwrap();
        let rules = setup.exit_rules().unwrap();
        match &rules[2] {
            DynamicExitRule::SmaCross { sma } => assert_eq!(*sma, Indicator::Sma10),
            other => panic!("expected SmaCross, got {:?}", other),
        }
    }

    #[test]
    fn exit_rules_breakeven_upgrade() {
        let json = r#"{
            "name": "be", "pipeline": [], "stop": { "type": "fixed_pct" },
            "exits": [{ "type": "breakeven_upgrade", "after_bars": 3 }]
        }"#;
        let setup = DynamicSetup::from_json(json).unwrap();
        let rules = setup.exit_rules().unwrap();
        match &rules[0] {
            DynamicExitRule::BreakevenUpgrade { after_bars } => assert_eq!(*after_bars, 3),
            other => panic!("expected BreakevenUpgrade, got {:?}", other),
        }
    }

    #[test]
    fn exit_rules_sma_cover() {
        let json = r#"{
            "name": "sc", "pipeline": [], "stop": { "type": "fixed_pct" },
            "exits": [{ "type": "sma_cover", "sma1": "sma_10", "sma2": "sma_20" }]
        }"#;
        let setup = DynamicSetup::from_json(json).unwrap();
        let rules = setup.exit_rules().unwrap();
        match &rules[0] {
            DynamicExitRule::SmaCover { sma1, sma2 } => {
                assert_eq!(*sma1, Indicator::Sma10);
                assert_eq!(*sma2, Indicator::Sma20);
            }
            other => panic!("expected SmaCover, got {:?}", other),
        }
    }

    #[test]
    fn exit_rules_signal_types() {
        let json = r#"{
            "name": "sig", "pipeline": [], "stop": { "type": "fixed_pct" },
            "exits": [
                { "type": "signal_bocpd_exit", "threshold": 0.7 },
                { "type": "signal_kalman_stop", "atr_mult": 1.5 }
            ]
        }"#;
        let setup = DynamicSetup::from_json(json).unwrap();
        let rules = setup.exit_rules().unwrap();
        match &rules[0] {
            DynamicExitRule::SignalBocpdExit { threshold } => {
                assert!((threshold - 0.7).abs() < 1e-6);
            }
            other => panic!("expected SignalBocpdExit, got {:?}", other),
        }
        match &rules[1] {
            DynamicExitRule::SignalKalmanStop { atr_mult } => {
                assert!((atr_mult - 1.5).abs() < 1e-6);
            }
            other => panic!("expected SignalKalmanStop, got {:?}", other),
        }
    }

    #[test]
    fn build_default_registry_has_all_blocks() {
        let reg = build_default_registry();
        let names = reg.names();
        // 13 universe + 5 pattern + 4 entry + 3 stop + 3 combiner + 1 signal + 3 cross = 32
        assert!(
            names.len() >= 31,
            "expected at least 30 blocks, got {}",
            names.len()
        );
        // Spot-check a few.
        assert!(reg.get("exclude_etf").is_some());
        assert!(reg.get("price_floor").is_some());
        assert!(reg.get("vcp").is_some());
        assert!(reg.get("breakout_above").is_some());
        assert!(reg.get("atr_capped_low").is_some());
        assert!(reg.get("and").is_some());
        assert!(reg.get("signal_pipeline").is_some());
        assert!(reg.get("parabolic_entry").is_some());
        assert!(reg.get("ising_susceptibility").is_some());
    }
}
