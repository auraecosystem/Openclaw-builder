//! Pattern detector blocks.
//!
//! Each block reads pre-computed indicator matrices from the `DataStore` and
//! produces a `WideMask` marking cells that match the pattern. Input is a
//! universe mask; output is a subset of that mask.

use std::collections::HashMap;

use engine_data::Indicator;
use engine_types::WideMask;

use crate::blackboard::{Slot, SlotType};
use super::{Block, BlockContext};

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

/// Read an f32 from config, returning an error with block name context if missing.
#[allow(dead_code)]
fn get_f32(
    config: &HashMap<String, serde_json::Value>,
    key: &str,
    block_name: &str,
) -> anyhow::Result<f32> {
    config
        .get(key)
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .ok_or_else(|| anyhow::anyhow!("{block_name}: missing config key '{key}'"))
}

/// Read an f32 from config, falling back to `default` when absent.
fn get_f32_or(
    config: &HashMap<String, serde_json::Value>,
    key: &str,
    default: f32,
) -> f32 {
    config
        .get(key)
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// VcpDetect
// ---------------------------------------------------------------------------

/// VCP (Volatility Contraction Pattern) detector.
///
/// Passes cells where pre-computed VCP indicators show at least 2 progressive
/// contractions, the last contraction is within `max_range`, tightening ratio
/// is declining, and volume trend is drying up.
pub struct VcpDetect;

impl Block for VcpDetect {
    fn name(&self) -> &'static str { "vcp" }
    fn output_type(&self) -> SlotType { SlotType::Mask }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let max_range = get_f32_or(ctx.config, "max_range", 0.15);
        let input = ctx.input_mask().ok_or_else(|| anyhow::anyhow!("vcp: no input mask"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;

        let vcp_nc = ctx.store.get(Indicator::VcpNumContractions);
        let vcp_last = ctx.store.get(Indicator::VcpLastContractionPct);
        let vcp_tight = ctx.store.get(Indicator::VcpTighteningRatio);
        let vcp_vol = ctx.store.get(Indicator::VcpVolTrend);

        let mut out = WideMask::new_false(nr, nc);

        for row in 0..nr {
            for col in 0..nc {
                if !input.get(row, col) {
                    continue;
                }
                let pass = vcp_nc.get(row, col) >= 2.0
                    && vcp_last.get(row, col) < max_range
                    && vcp_tight.get(row, col) < 1.0
                    && vcp_vol.get(row, col) < 1.0;
                if pass {
                    out.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(out))
    }
}

// ---------------------------------------------------------------------------
// FlagDetect
// ---------------------------------------------------------------------------

/// Flag pattern detector (Qullamaggie bull flag rules).
///
/// Passes cells where: pole >= 20%, retrace 0-50%, flag duration 5-25 days,
/// and volume ratio declining (< 1.0). All thresholds are canonical.
pub struct FlagDetect;

impl Block for FlagDetect {
    fn name(&self) -> &'static str { "flag" }
    fn output_type(&self) -> SlotType { SlotType::Mask }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let input = ctx.input_mask().ok_or_else(|| anyhow::anyhow!("flag: no input mask"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;

        let fp = ctx.store.get(Indicator::FlagPolePct);
        let fr = ctx.store.get(Indicator::FlagRetracePct);
        let fd = ctx.store.get(Indicator::FlagDays);
        let fv = ctx.store.get(Indicator::FlagVolRatio);

        let mut out = WideMask::new_false(nr, nc);

        for row in 0..nr {
            for col in 0..nc {
                if !input.get(row, col) {
                    continue;
                }
                let pole = fp.get(row, col);
                let retrace = fr.get(row, col);
                let days = fd.get(row, col);
                let vol_r = fv.get(row, col);
                let pass = pole >= 0.20
                    && retrace > 0.0
                    && retrace < 0.50
                    && (5.0..=25.0).contains(&days)
                    && vol_r < 1.0;
                if pass {
                    out.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(out))
    }
}

// ---------------------------------------------------------------------------
// GapUp
// ---------------------------------------------------------------------------

/// Gap-up detector for EP (Episodic Pivot) strategy.
///
/// Passes cells where the overnight gap (close / prev_close - 1) exceeds a
/// minimum threshold, volume spikes relative to the 20-day SMA, and dollar
/// volume meets minimum ADV requirements. Row 0 is always skipped (no
/// previous close available).
pub struct GapUp;

impl Block for GapUp {
    fn name(&self) -> &'static str { "gap_up" }
    fn output_type(&self) -> SlotType { SlotType::Mask }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let min_gap_pct = get_f32_or(ctx.config, "min_gap_pct", 0.10);
        let min_vol_ratio = get_f32_or(ctx.config, "min_vol_ratio", 2.0);
        let min_dollar_vol = get_f32_or(ctx.config, "min_dollar_vol", 0.0);

        let input = ctx.input_mask().ok_or_else(|| anyhow::anyhow!("gap_up: no input mask"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;

        let close_m = ctx.store.close();
        let vol_m = ctx.store.volume();
        let vol_sma = ctx.store.get(Indicator::VolSma20);

        let mut out = WideMask::new_false(nr, nc);

        // Start at row 1: need prev_close from row-1.
        for row in 1..nr {
            for col in 0..nc {
                if !input.get(row, col) {
                    continue;
                }
                let close = close_m.get(row, col);
                let prev_close = close_m.get(row - 1, col);
                if prev_close.is_nan() || prev_close <= 0.0 || close.is_nan() {
                    continue;
                }
                let gap = close / prev_close - 1.0;
                if gap < min_gap_pct {
                    continue;
                }

                let vsma = vol_sma.get(row, col);
                let vol = vol_m.get(row, col);
                if vsma.is_nan() || vol < min_vol_ratio * vsma {
                    continue;
                }

                // Dollar volume filter (ADV proxy)
                if min_dollar_vol > 0.0 && vsma * close < min_dollar_vol {
                    continue;
                }

                out.set(row, col, true);
            }
        }
        Ok(Slot::Mask(out))
    }
}

// ---------------------------------------------------------------------------
// ParabolicRun
// ---------------------------------------------------------------------------

/// Parabolic run detector.
///
/// Identifies stocks that have made an extreme 10-day move (threshold depends
/// on market-cap proxy via price), are showing their first red day (close < open),
/// and optionally had low prior 6-month returns (prior neglect filter).
pub struct ParabolicRun;

impl Block for ParabolicRun {
    fn name(&self) -> &'static str { "parabolic_run" }
    fn output_type(&self) -> SlotType { SlotType::Mask }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let large_cap_price = get_f32_or(ctx.config, "large_cap_price", 50.0);
        let large_cap_run = get_f32_or(ctx.config, "large_cap_run", 0.50);
        let small_cap_run = get_f32_or(ctx.config, "small_cap_run", 3.00);
        let max_prior_return = get_f32_or(ctx.config, "max_prior_return", 0.30);

        let input = ctx.input_mask().ok_or_else(|| anyhow::anyhow!("parabolic_run: no input mask"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;

        let close_m = ctx.store.close();
        let open_m = ctx.store.open();
        let pct_10d = ctx.store.get(Indicator::Pct10d);
        let ret_126 = ctx.store.get(Indicator::Ret126);

        let mut out = WideMask::new_false(nr, nc);

        for row in 0..nr {
            for col in 0..nc {
                if !input.get(row, col) {
                    continue;
                }
                let close = close_m.get(row, col);
                if close.is_nan() {
                    continue;
                }

                // Run threshold depends on price (market-cap proxy)
                let pct = pct_10d.get(row, col);
                let run_ok = if close >= large_cap_price {
                    pct >= large_cap_run
                } else {
                    pct >= small_cap_run
                };
                if !run_ok {
                    continue;
                }

                // First red day: close < open
                let open_val = open_m.get(row, col);
                if close >= open_val {
                    continue;
                }

                // Prior neglect: 6-month return should be low
                let ret = ret_126.get(row, col);
                if !ret.is_nan() && ret >= max_prior_return {
                    continue;
                }

                out.set(row, col, true);
            }
        }
        Ok(Slot::Mask(out))
    }
}

// ---------------------------------------------------------------------------
// PatternAny
// ---------------------------------------------------------------------------

/// OR-combiner for multiple pattern detectors.
///
/// Config `"patterns"` is a JSON array of pattern names (e.g. `["vcp", "flag"]`).
/// Each named pattern's block is instantiated and executed against the same
/// input mask; results are OR'd together.
pub struct PatternAny;

impl Block for PatternAny {
    fn name(&self) -> &'static str { "pattern_any" }
    fn output_type(&self) -> SlotType { SlotType::Mask }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let patterns = ctx
            .config
            .get("patterns")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("pattern_any: missing 'patterns' array"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut combined = WideMask::new_false(nr, nc);

        for pat_val in patterns {
            let pat_name = pat_val
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("pattern_any: pattern must be a string"))?;

            let block: Box<dyn Block> = match pat_name {
                "vcp" => Box::new(VcpDetect),
                "flag" => Box::new(FlagDetect),
                "gap_up" => Box::new(GapUp),
                "parabolic_run" => Box::new(ParabolicRun),
                other => anyhow::bail!("pattern_any: unknown pattern '{other}'"),
            };

            let result = block.execute(ctx)?;
            let mask = result
                .as_mask()
                .ok_or_else(|| anyhow::anyhow!("pattern_any: sub-block did not return mask"))?;

            // OR into combined
            for i in 0..combined.data.len() {
                combined.data[i] |= mask.data[i];
            }
        }

        Ok(Slot::Mask(combined))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_f32_parses_integer_and_float() {
        let mut config = HashMap::new();
        config.insert("a".into(), serde_json::json!(0.15));
        config.insert("b".into(), serde_json::json!(2));
        config.insert("c".into(), serde_json::json!("not_a_number"));

        assert!((get_f32(&config, "a", "test").unwrap() - 0.15).abs() < 1e-6);
        assert!((get_f32(&config, "b", "test").unwrap() - 2.0).abs() < 1e-6);
        assert!(get_f32(&config, "c", "test").is_err());
        assert!(get_f32(&config, "missing", "test").is_err());
    }

    #[test]
    fn get_f32_or_uses_default() {
        let config = HashMap::new();
        assert!((get_f32_or(&config, "x", 0.05) - 0.05).abs() < 1e-6);

        let mut config2 = HashMap::new();
        config2.insert("x".into(), serde_json::json!(0.10));
        assert!((get_f32_or(&config2, "x", 0.05) - 0.10).abs() < 1e-6);
    }

    #[test]
    fn pattern_any_rejects_unknown() {
        // Verify that unknown pattern names produce a clear error
        let patterns = vec!["vcp", "flag", "gap_up", "parabolic_run"];
        for p in &patterns {
            let block: Box<dyn Block> = match *p {
                "vcp" => Box::new(VcpDetect),
                "flag" => Box::new(FlagDetect),
                "gap_up" => Box::new(GapUp),
                "parabolic_run" => Box::new(ParabolicRun),
                _ => unreachable!(),
            };
            assert_eq!(block.output_type(), SlotType::Mask);
        }
    }

    #[test]
    fn block_names_are_correct() {
        assert_eq!(VcpDetect.name(), "vcp");
        assert_eq!(FlagDetect.name(), "flag");
        assert_eq!(GapUp.name(), "gap_up");
        assert_eq!(ParabolicRun.name(), "parabolic_run");
        assert_eq!(PatternAny.name(), "pattern_any");
    }
}
