//! Universe filter blocks: stateless pipeline steps that produce a `WideMask`
//! selecting which (row, col) cells pass the filter. Each block is extracted
//! from the fused `breakout_filter()` in `strategy/breakout.rs` so that
//! strategies can compose them via JSON config.

use std::collections::HashMap;

use engine_data::Indicator;
use engine_types::WideMask;

use crate::blackboard::{Slot, SlotType};
use super::{Block, BlockContext, CmpOp, FusableSpec};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract an f32 from the JSON config map, with a clear error on missing/bad values.
fn get_f32(
    config: &HashMap<String, serde_json::Value>,
    key: &str,
    block_name: &str,
) -> anyhow::Result<f32> {
    config
        .get(key)
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .ok_or_else(|| anyhow::anyhow!("{block_name}: missing or invalid '{key}'"))
}

/// Extract a usize from the JSON config map.
fn get_usize(
    config: &HashMap<String, serde_json::Value>,
    key: &str,
    block_name: &str,
) -> anyhow::Result<usize> {
    config
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .ok_or_else(|| anyhow::anyhow!("{block_name}: missing or invalid '{key}'"))
}

/// Map a string name to an `Indicator` enum variant.
fn parse_indicator(s: &str) -> anyhow::Result<Indicator> {
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

/// Map a timeframe short-code ("1m", "3m", "6m") to the matching RS pctrank indicator.
fn rs_indicator_for_timeframe(tf: &str) -> anyhow::Result<Indicator> {
    match tf {
        "1m" => Ok(Indicator::RsPctrank1m),
        "3m" => Ok(Indicator::RsPctrank3m),
        "6m" => Ok(Indicator::RsPctrank6m),
        other => anyhow::bail!("rs_percentile: unknown timeframe '{other}'"),
    }
}

// ---------------------------------------------------------------------------
// 1. ExcludeEtf
// ---------------------------------------------------------------------------

/// Excludes ETF tickers based on the per-column `etf_cols` flag in the DataStore axes.
/// NOT fusable: operates at column level, not indicator-based.
pub struct ExcludeEtf;

impl Block for ExcludeEtf {
    fn name(&self) -> &'static str { "exclude_etf" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if ctx.store.axes.etf_cols[col] {
                    continue;
                }
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                mask.set(row, col, true);
            }
        }
        Ok(Slot::Mask(mask))
    }
}

// ---------------------------------------------------------------------------
// 2. PriceFloor
// ---------------------------------------------------------------------------

/// Passes cells where `close > config["min"]`.
pub struct PriceFloor;

impl Block for PriceFloor {
    fn name(&self) -> &'static str { "price_floor" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let min = get_f32(&ctx.config, "min", "price_floor")?;
        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);
        let close = ctx.store.close();

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let v = close.get(row, col);
                if !v.is_nan() && v > min {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }

    fn fusable_spec(&self) -> Option<FusableSpec> {
        Some(FusableSpec {
            indicator: Indicator::Close,
            op: CmpOp::Gt,
            threshold_key: "min".to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// 3. VolumeFloor
// ---------------------------------------------------------------------------

/// Passes cells where `vol_sma_20 > config["min"]`.
pub struct VolumeFloor;

impl Block for VolumeFloor {
    fn name(&self) -> &'static str { "volume_floor" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let min = get_f32(&ctx.config, "min", "volume_floor")?;
        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);
        let vol_sma = ctx.store.get(Indicator::VolSma20);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let v = vol_sma.get(row, col);
                if !v.is_nan() && v > min {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }

    fn fusable_spec(&self) -> Option<FusableSpec> {
        Some(FusableSpec {
            indicator: Indicator::VolSma20,
            op: CmpOp::Gt,
            threshold_key: "min".to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// 4. AdvFloor (Average Dollar Volume)
// ---------------------------------------------------------------------------

/// Passes cells where `vol_sma_20 * close > config["min"]`.
/// NOT fusable: requires two indicators multiplied together.
pub struct AdvFloor;

impl Block for AdvFloor {
    fn name(&self) -> &'static str { "adv_floor" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let min = get_f32(&ctx.config, "min", "adv_floor")?;
        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);
        let close = ctx.store.close();
        let vol_sma = ctx.store.get(Indicator::VolSma20);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let c = close.get(row, col);
                let v = vol_sma.get(row, col);
                if !c.is_nan() && !v.is_nan() && (v * c) > min {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }
}

// ---------------------------------------------------------------------------
// 5. IndicatorGte (generic >= threshold)
// ---------------------------------------------------------------------------

/// Generic filter: `indicator >= config["value"]`.
/// Config: `"indicator"` (string name), `"value"` (f32 threshold).
pub struct IndicatorGte;

impl Block for IndicatorGte {
    fn name(&self) -> &'static str { "indicator_gte" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let ind_name = ctx.config.get("indicator")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("indicator_gte: missing 'indicator'"))?;
        let ind = parse_indicator(ind_name)?;
        let threshold = get_f32(&ctx.config, "value", "indicator_gte")?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);
        let mat = ctx.store.get(ind);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let v = mat.get(row, col);
                if !v.is_nan() && v >= threshold {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }

    fn fusable_spec(&self) -> Option<FusableSpec> {
        // Fusable spec is determined at planning time from config, not here.
        // The planner reads config["indicator"] and config["value"] to build
        // the spec. We return None because the indicator is not known statically.
        None
    }
}

// ---------------------------------------------------------------------------
// 6. IndicatorLte (generic <= threshold)
// ---------------------------------------------------------------------------

/// Generic filter: `indicator <= config["value"]`.
/// Config: `"indicator"` (string name), `"value"` (f32 threshold),
/// `"pass_nan"` (bool, default false) — when true, NaN values pass the filter.
pub struct IndicatorLte;

impl Block for IndicatorLte {
    fn name(&self) -> &'static str { "indicator_lte" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let ind_name = ctx.config.get("indicator")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("indicator_lte: missing 'indicator'"))?;
        let ind = parse_indicator(ind_name)?;
        let threshold = get_f32(&ctx.config, "value", "indicator_lte")?;
        let pass_nan = ctx.config.get("pass_nan")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);
        let mat = ctx.store.get(ind);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let v = mat.get(row, col);
                if v.is_nan() {
                    if pass_nan { mask.set(row, col, true); }
                } else if v <= threshold {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }

    fn fusable_spec(&self) -> Option<FusableSpec> {
        None
    }
}

// ---------------------------------------------------------------------------
// 7. RsPercentile
// ---------------------------------------------------------------------------

/// Passes cells where RS pctrank across all configured timeframes exceeds the threshold.
/// Config: `"timeframes"` (array of "1m"/"3m"/"6m"), `"min_pct"` (f32).
/// NOT fusable: checks multiple indicators per cell.
pub struct RsPercentile;

impl Block for RsPercentile {
    fn name(&self) -> &'static str { "rs_percentile" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let min_pct = get_f32(&ctx.config, "min_pct", "rs_percentile")?;
        // RS pctrank values are 0..1 where 1.0 = top percentile.
        // The breakout filter checks `rs >= (1.0 - rs_pct)`, i.e. the threshold
        // is the complement. Here we expose min_pct directly as the threshold
        // (caller passes 0.80 meaning "top 20%").
        let threshold = 1.0 - min_pct;

        let timeframes = ctx.config.get("timeframes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("rs_percentile: missing 'timeframes' array"))?;

        // Resolve indicator matrices up front to avoid per-cell string parsing.
        let indicators: Vec<Indicator> = timeframes.iter()
            .map(|v| {
                let s = v.as_str().unwrap_or("");
                rs_indicator_for_timeframe(s)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let matrices: Vec<&engine_types::WideMatrix> = indicators.iter()
            .map(|ind| ctx.store.get(*ind))
            .collect();

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let pass = matrices.iter().all(|m| {
                    let v = m.get(row, col);
                    !v.is_nan() && v >= threshold
                });
                if pass {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }
}

// ---------------------------------------------------------------------------
// 8. Near52wHigh
// ---------------------------------------------------------------------------

/// Passes cells where `dist_52w <= config["max_dist"]`.
pub struct Near52wHigh;

impl Block for Near52wHigh {
    fn name(&self) -> &'static str { "near_52w_high" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let max_dist = get_f32(&ctx.config, "max_dist", "near_52w_high")?;
        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);
        let dist = ctx.store.get(Indicator::Dist52w);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let v = dist.get(row, col);
                if !v.is_nan() && v <= max_dist {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }

    fn fusable_spec(&self) -> Option<FusableSpec> {
        Some(FusableSpec {
            indicator: Indicator::Dist52w,
            op: CmpOp::Le,
            threshold_key: "max_dist".to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// 9. PriorMove
// ---------------------------------------------------------------------------

/// Passes cells where `ret_63 >= config["min"]`.
pub struct PriorMove;

impl Block for PriorMove {
    fn name(&self) -> &'static str { "prior_move" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let min = get_f32(&ctx.config, "min", "prior_move")?;
        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);
        let ret = ctx.store.get(Indicator::Ret63);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let v = ret.get(row, col);
                if !v.is_nan() && v >= min {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }

    fn fusable_spec(&self) -> Option<FusableSpec> {
        Some(FusableSpec {
            indicator: Indicator::Ret63,
            op: CmpOp::Ge,
            threshold_key: "min".to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// 10. AdrFloor (Average Daily Range %)
// ---------------------------------------------------------------------------

/// Passes cells where `atr_14 / close >= config["min_pct"]`.
/// NOT fusable: ratio of two indicator values.
pub struct AdrFloor;

impl Block for AdrFloor {
    fn name(&self) -> &'static str { "adr_floor" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let min_pct = get_f32(&ctx.config, "min_pct", "adr_floor")?;
        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);
        let close = ctx.store.close();
        let atr = ctx.store.get(Indicator::Atr14);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let a = atr.get(row, col);
                let c = close.get(row, col);
                if !a.is_nan() && !c.is_nan() && c > 0.0 && (a / c) >= min_pct {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }
}

// ---------------------------------------------------------------------------
// 11. ExtensionCap
// ---------------------------------------------------------------------------

/// Passes cells where `(close - consol_high) <= config["max_atr_above"] * atr_14`.
/// Prevents entries on stocks that have already extended too far above their base.
/// NOT fusable: complex expression involving multiple indicators.
pub struct ExtensionCap;

impl Block for ExtensionCap {
    fn name(&self) -> &'static str { "extension_cap" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let max_atr_above = get_f32(&ctx.config, "max_atr_above", "extension_cap")?;
        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);
        let close = ctx.store.close();
        let consol_high = ctx.store.get(Indicator::ConsolHigh);
        let atr = ctx.store.get(Indicator::Atr14);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let c = close.get(row, col);
                let ch = consol_high.get(row, col);
                let a = atr.get(row, col);
                // Skip if consol_high or ATR is NaN — cannot evaluate the condition.
                if ch.is_nan() || a.is_nan() {
                    // When data is missing, pass the cell (matches breakout.rs
                    // behavior where NaN consol_high does not reject).
                    mask.set(row, col, true);
                    continue;
                }
                // ATR <= 0 means we can't check extension — pass the cell.
                if a <= 0.0 || (c - ch) <= max_atr_above * a {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }
}

// ---------------------------------------------------------------------------
// 12. Consolidation
// ---------------------------------------------------------------------------

/// Lookback check: N consecutive days where `vcp_last_contraction_pct < max_range`.
/// Config: `"min_days"` (usize), `"max_range"` (f32).
/// NOT fusable: requires lookback window.
///
/// Reference: breakout.rs lines 140-153.
pub struct Consolidation;

impl Block for Consolidation {
    fn name(&self) -> &'static str { "consolidation" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let min_days = get_usize(&ctx.config, "min_days", "consolidation")?;
        let max_range = get_f32(&ctx.config, "max_range", "consolidation")?;
        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let mut mask = WideMask::new_false(nr, nc);

        // A min_days of 0 means no consolidation check — pass everything.
        if min_days == 0 {
            for row in ctx.range.clone() {
                for col in 0..nc {
                    if let Some(inp) = ctx.input_mask() {
                        if !inp.get(row, col) { continue; }
                    }
                    mask.set(row, col, true);
                }
            }
            return Ok(Slot::Mask(mask));
        }

        let vcp_last = ctx.store.get(Indicator::VcpLastContractionPct);

        for row in ctx.range.clone() {
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                let needed = min_days;
                let start = row.saturating_sub(needed - 1);
                let count = (start..=row)
                    .rev()
                    .take_while(|&r| {
                        let v = vcp_last.get(r, col);
                        !v.is_nan() && v < max_range
                    })
                    .count();
                if count >= needed {
                    mask.set(row, col, true);
                }
            }
        }
        Ok(Slot::Mask(mask))
    }
}

// ---------------------------------------------------------------------------
// 13. RegimeEma
// ---------------------------------------------------------------------------

/// Per-row regime filter: benchmark EMA10 > EMA20.
/// Uses `store.axes.spy_col` for the benchmark column. When the regime is off,
/// the entire row is blocked. If no spy_col is set, all rows pass.
/// NOT fusable: per-row computation, not per-cell indicator threshold.
///
/// Reference: breakout.rs `compute_regime()` function (lines 27-61).
pub struct RegimeEma;

impl Block for RegimeEma {
    fn name(&self) -> &'static str { "regime_ema" }
    fn output_type(&self) -> SlotType { SlotType::Mask }
    fn input_type(&self) -> Option<SlotType> { None }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let regime = compute_regime(ctx.store);
        let mut mask = WideMask::new_false(nr, nc);

        for row in ctx.range.clone() {
            if !regime[row] {
                continue;
            }
            for col in 0..nc {
                if let Some(inp) = ctx.input_mask() {
                    if !inp.get(row, col) { continue; }
                }
                mask.set(row, col, true);
            }
        }
        Ok(Slot::Mask(mask))
    }
}

/// Compute per-row regime using benchmark EMA10 vs EMA20.
/// Extracted from `strategy/breakout.rs:compute_regime()`.
fn compute_regime(store: &engine_data::DataStore) -> Vec<bool> {
    let nr = store.axes.n_rows;
    let mut regime = vec![true; nr];

    let Some(bench_col) = store.axes.spy_col else {
        return regime;
    };

    let alpha10: f64 = 2.0 / 11.0;
    let alpha20: f64 = 2.0 / 21.0;
    let mut ema10: f64 = f64::NAN;
    let mut ema20: f64 = f64::NAN;
    let mut count = 0usize;

    for (row, regime_val) in regime.iter_mut().enumerate() {
        let val = store.close().get(row, bench_col) as f64;
        if val.is_nan() {
            continue;
        }
        count += 1;

        if count == 1 {
            ema10 = val;
            ema20 = val;
        } else {
            ema10 = alpha10 * val + (1.0 - alpha10) * ema10;
            ema20 = alpha20 * val + (1.0 - alpha20) * ema20;
        }

        // EMA needs at least 20 bars to be meaningful.
        *regime_val = if count >= 20 { ema10 >= ema20 } else { true };
    }
    regime
}

// ---------------------------------------------------------------------------
// Public list of all universe blocks (for registry registration).
// ---------------------------------------------------------------------------

/// Return boxed instances of all universe filter blocks for registry registration.
pub fn all_blocks() -> Vec<Box<dyn Block>> {
    vec![
        Box::new(ExcludeEtf),
        Box::new(PriceFloor),
        Box::new(VolumeFloor),
        Box::new(AdvFloor),
        Box::new(IndicatorGte),
        Box::new(IndicatorLte),
        Box::new(RsPercentile),
        Box::new(Near52wHigh),
        Box::new(PriorMove),
        Box::new(AdrFloor),
        Box::new(ExtensionCap),
        Box::new(Consolidation),
        Box::new(RegimeEma),
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use engine_data::{DataStore, Indicator};
    use engine_types::WideMatrix;

    use crate::blackboard::Blackboard;

    use strum::EnumCount;
    const INDICATOR_COUNT: usize = engine_data::Indicator::COUNT;

    /// Build a minimal DataStore with all indicator matrices initialized to `fill`
    /// (or NaN), then apply per-indicator overrides.
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

    fn make_ctx<'a>(
        store: &'a DataStore,
        config: &'a HashMap<String, serde_json::Value>,
        blackboard: &'a Blackboard,
    ) -> BlockContext<'a> {
        BlockContext {
            store,
            config,
            range: 0..store.axes.n_rows,
            blackboard,
            input_id: None,
        }
    }

    // -- Helper tests --

    #[test]
    fn test_get_f32_valid() {
        let mut cfg = HashMap::new();
        cfg.insert("min".into(), serde_json::json!(5.0));
        assert!((get_f32(&cfg, "min", "test").unwrap() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_get_f32_missing() {
        let cfg = HashMap::new();
        assert!(get_f32(&cfg, "min", "test").is_err());
    }

    #[test]
    fn test_get_f32_wrong_type() {
        let mut cfg = HashMap::new();
        cfg.insert("min".into(), serde_json::json!("not a number"));
        assert!(get_f32(&cfg, "min", "test").is_err());
    }

    #[test]
    fn test_parse_indicator_valid() {
        assert_eq!(parse_indicator("close").unwrap(), Indicator::Close);
        assert_eq!(parse_indicator("atr_14").unwrap(), Indicator::Atr14);
        assert_eq!(parse_indicator("vcp_vol_trend").unwrap(), Indicator::VcpVolTrend);
    }

    #[test]
    fn test_parse_indicator_unknown() {
        assert!(parse_indicator("nonexistent").is_err());
    }

    // -- Block tests --

    #[test]
    fn test_price_floor() {
        // 3x3 matrix: row 0 = [1, 10, 5], row 1 = [20, 3, 8], row 2 = [NaN, 15, 6]
        let close_data = vec![1.0, 10.0, 5.0, 20.0, 3.0, 8.0, f32::NAN, 15.0, 6.0];
        let store = make_store(3, 3, &[(Indicator::Close, close_data)]);
        let mut cfg = HashMap::new();
        cfg.insert("min".into(), serde_json::json!(5.0));
        let bb = Blackboard::new();
        let ctx = make_ctx(&store, &cfg, &bb);

        let result = PriceFloor.execute(&ctx).unwrap();
        let mask = result.as_mask().unwrap();

        // Values > 5.0: (0,1)=10, (1,0)=20, (1,2)=8, (2,1)=15, (2,2)=6
        assert!(!mask.get(0, 0)); // 1.0 <= 5.0
        assert!(mask.get(0, 1));  // 10.0 > 5.0
        assert!(!mask.get(0, 2)); // 5.0 not > 5.0 (strict)
        assert!(mask.get(1, 0));  // 20.0 > 5.0
        assert!(!mask.get(1, 1)); // 3.0 <= 5.0
        assert!(mask.get(1, 2));  // 8.0 > 5.0
        assert!(!mask.get(2, 0)); // NaN
        assert!(mask.get(2, 1));  // 15.0 > 5.0
        assert!(mask.get(2, 2));  // 6.0 > 5.0
    }

    #[test]
    fn test_exclude_etf() {
        // 2x3 matrix, col 1 is ETF
        let close_data = vec![10.0; 6];
        let mut store = make_store(2, 3, &[(Indicator::Close, close_data)]);
        store.axes.etf_cols[1] = true;

        let cfg = HashMap::new();
        let bb = Blackboard::new();
        let ctx = make_ctx(&store, &cfg, &bb);

        let result = ExcludeEtf.execute(&ctx).unwrap();
        let mask = result.as_mask().unwrap();

        assert!(mask.get(0, 0));
        assert!(!mask.get(0, 1)); // ETF column
        assert!(mask.get(0, 2));
        assert!(mask.get(1, 0));
        assert!(!mask.get(1, 1)); // ETF column
        assert!(mask.get(1, 2));
    }

    #[test]
    fn test_indicator_gte() {
        let ret_data = vec![0.1, 0.5, 0.3, 0.8, 0.05, 0.6];
        let store = make_store(2, 3, &[(Indicator::Ret63, ret_data)]);
        let mut cfg = HashMap::new();
        cfg.insert("indicator".into(), serde_json::json!("ret_63"));
        cfg.insert("value".into(), serde_json::json!(0.3));
        let bb = Blackboard::new();
        let ctx = make_ctx(&store, &cfg, &bb);

        let result = IndicatorGte.execute(&ctx).unwrap();
        let mask = result.as_mask().unwrap();

        assert!(!mask.get(0, 0)); // 0.1 < 0.3
        assert!(mask.get(0, 1));  // 0.5 >= 0.3
        assert!(mask.get(0, 2));  // 0.3 >= 0.3
        assert!(mask.get(1, 0));  // 0.8 >= 0.3
        assert!(!mask.get(1, 1)); // 0.05 < 0.3
        assert!(mask.get(1, 2));  // 0.6 >= 0.3
    }

    #[test]
    fn test_rs_percentile_mixed() {
        // 2 rows, 2 cols. RS pctrank values for 1m/3m/6m.
        // min_pct = 0.80 means threshold = 1.0 - 0.80 = 0.20
        // Cell passes when ALL timeframe values >= 0.20.
        let rs_1m = vec![0.9, 0.1, 0.8, 0.5];
        let rs_3m = vec![0.8, 0.3, 0.7, 0.6];
        let rs_6m = vec![0.7, 0.5, 0.2, 0.4];
        let store = make_store(2, 2, &[
            (Indicator::RsPctrank1m, rs_1m),
            (Indicator::RsPctrank3m, rs_3m),
            (Indicator::RsPctrank6m, rs_6m),
        ]);
        let mut cfg = HashMap::new();
        cfg.insert("min_pct".into(), serde_json::json!(0.80));
        cfg.insert("timeframes".into(), serde_json::json!(["1m", "3m", "6m"]));
        let bb = Blackboard::new();
        let ctx = make_ctx(&store, &cfg, &bb);

        let result = RsPercentile.execute(&ctx).unwrap();
        let mask = result.as_mask().unwrap();

        // (0,0): 0.9>=0.2, 0.8>=0.2, 0.7>=0.2 -> pass
        assert!(mask.get(0, 0));
        // (0,1): 0.1<0.2 -> fail
        assert!(!mask.get(0, 1));
        // (1,0): 0.8>=0.2, 0.7>=0.2, 0.2>=0.2 -> pass
        assert!(mask.get(1, 0));
        // (1,1): 0.5>=0.2, 0.6>=0.2, 0.4>=0.2 -> pass
        assert!(mask.get(1, 1));
    }

    #[test]
    fn test_consolidation_lookback() {
        // 5 rows, 1 col. VCP last contraction pct values.
        // min_days=3, max_range=0.10
        // Row 4 lookback: rows 2,3,4 must all have vcp_last < 0.10
        let vcp_data = vec![0.20, 0.15, 0.05, 0.03, 0.08];
        let store = make_store(5, 1, &[(Indicator::VcpLastContractionPct, vcp_data)]);
        let mut cfg = HashMap::new();
        cfg.insert("min_days".into(), serde_json::json!(3));
        cfg.insert("max_range".into(), serde_json::json!(0.10));
        let bb = Blackboard::new();
        let ctx = make_ctx(&store, &cfg, &bb);

        let result = Consolidation.execute(&ctx).unwrap();
        let mask = result.as_mask().unwrap();

        // Row 0: lookback [0], only 1 day < 0.10? val=0.20, no -> false
        assert!(!mask.get(0, 0));
        // Row 1: lookback [0,1], val=0.15 >= 0.10 -> fail at row 1 itself
        assert!(!mask.get(1, 0));
        // Row 2: lookback [0,1,2], need 3 consecutive. row2=0.05<0.10, row1=0.15>=0.10 -> only 1 -> fail
        assert!(!mask.get(2, 0));
        // Row 3: lookback [1,2,3]. row3=0.03, row2=0.05, row1=0.15>=0.10 -> 2 consecutive -> fail
        assert!(!mask.get(3, 0));
        // Row 4: lookback [2,3,4]. row4=0.08, row3=0.03, row2=0.05 -> 3 consecutive -> pass
        assert!(mask.get(4, 0));
    }

    #[test]
    fn test_regime_ema_no_spy() {
        // No spy_col => all rows pass.
        let close_data = vec![100.0; 6];
        let store = make_store(3, 2, &[(Indicator::Close, close_data)]);
        let cfg = HashMap::new();
        let bb = Blackboard::new();
        let ctx = make_ctx(&store, &cfg, &bb);

        let result = RegimeEma.execute(&ctx).unwrap();
        let mask = result.as_mask().unwrap();

        for row in 0..3 {
            for col in 0..2 {
                assert!(mask.get(row, col));
            }
        }
    }

    #[test]
    fn test_regime_ema_with_spy() {
        // 30 rows, 2 cols. spy_col=0. Benchmark starts high then crashes.
        // First 20 rows: stable at 100.0 -> regime on (warmup).
        // Rows 20-29: price drops to 50.0 -> EMA10 drops below EMA20 -> regime off.
        let n_rows = 30;
        let n_cols = 2;
        let mut close_data = vec![100.0; n_rows * n_cols];
        // Drop spy column (col 0) price from row 20 onward.
        for row in 20..n_rows {
            close_data[row * n_cols] = 50.0;
        }

        let mut store = make_store(n_rows, n_cols, &[(Indicator::Close, close_data)]);
        store.axes.spy_col = Some(0);

        let cfg = HashMap::new();
        let bb = Blackboard::new();
        let ctx = BlockContext {
            store: &store,
            config: &cfg,
            range: 0..n_rows,
            blackboard: &bb,
            input_id: None,
        };

        let result = RegimeEma.execute(&ctx).unwrap();
        let mask = result.as_mask().unwrap();

        // First 20 rows should all pass (warmup period, stable price).
        for row in 0..20 {
            assert!(mask.get(row, 1), "row {row} should pass during warmup");
        }

        // After some rows of crashing, EMA10 should drop below EMA20
        // and regime should turn off. Check the last row.
        // With alpha10=2/11 and 10 bars at 50.0 after 20 bars at 100.0,
        // EMA10 responds faster and will be below EMA20.
        assert!(!mask.get(29, 1), "row 29 should be blocked by crashed regime");
    }

    #[test]
    fn test_extension_cap_nan_passthrough() {
        // When consol_high is NaN, the cell should pass (matches breakout.rs behavior).
        let close_data = vec![100.0, 200.0];
        let consol_data = vec![f32::NAN, 180.0];
        let atr_data = vec![5.0, 10.0];
        let store = make_store(1, 2, &[
            (Indicator::Close, close_data),
            (Indicator::ConsolHigh, consol_data),
            (Indicator::Atr14, atr_data),
        ]);
        let mut cfg = HashMap::new();
        cfg.insert("max_atr_above".into(), serde_json::json!(1.0));
        let bb = Blackboard::new();
        let ctx = make_ctx(&store, &cfg, &bb);

        let result = ExtensionCap.execute(&ctx).unwrap();
        let mask = result.as_mask().unwrap();

        // (0,0): consol_high=NaN -> pass
        assert!(mask.get(0, 0));
        // (0,1): close=200, ch=180, atr=10. (200-180)=20 > 1.0*10=10 -> fail
        assert!(!mask.get(0, 1));
    }

    #[test]
    fn test_all_blocks_returns_13() {
        let blocks = all_blocks();
        assert_eq!(blocks.len(), 13);
    }

    #[test]
    fn test_fusable_blocks() {
        // Verify which blocks declare themselves fusable.
        assert!(PriceFloor.fusable_spec().is_some());
        assert!(VolumeFloor.fusable_spec().is_some());
        assert!(Near52wHigh.fusable_spec().is_some());
        assert!(PriorMove.fusable_spec().is_some());

        // Non-fusable blocks.
        assert!(ExcludeEtf.fusable_spec().is_none());
        assert!(AdvFloor.fusable_spec().is_none());
        assert!(RsPercentile.fusable_spec().is_none());
        assert!(AdrFloor.fusable_spec().is_none());
        assert!(ExtensionCap.fusable_spec().is_none());
        assert!(Consolidation.fusable_spec().is_none());
        assert!(RegimeEma.fusable_spec().is_none());
        assert!(IndicatorGte.fusable_spec().is_none());
        assert!(IndicatorLte.fusable_spec().is_none());
    }
}
