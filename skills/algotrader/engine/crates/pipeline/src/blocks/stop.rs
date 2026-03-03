//! Stop generator blocks.
//!
//! Each stop block reads from the `DataStore` and an input `WideMask`, and
//! produces a `WideMatrix` of stop prices. Cells outside the input mask are
//! left as `NAN`.

use std::collections::HashMap;

use engine_data::Indicator;
use engine_types::WideMatrix;

use crate::blackboard::{Slot, SlotType};
use super::{Block, BlockContext};

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

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
// AtrCappedLow
// ---------------------------------------------------------------------------

/// ATR-capped stop: low-of-day, but no more than 1x ATR below close.
///
/// This is the standard stop from the breakout strategy. Falls back to
/// `close * (1 - fallback_pct)` when ATR or low is unavailable.
///
/// Config: `"fallback_pct"` (f32, default 0.05).
pub struct AtrCappedLow;

impl Block for AtrCappedLow {
    fn name(&self) -> &'static str { "atr_capped_low" }
    fn output_type(&self) -> SlotType { SlotType::Matrix }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let fallback_pct = get_f32_or(ctx.config, "fallback_pct", 0.05);
        let input = ctx.input_mask().ok_or_else(|| anyhow::anyhow!("atr_capped_low: no input mask"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let all_cols: Vec<u32> = (0..nc as u32).collect();

        let close_m = ctx.store.close();
        let low_m = ctx.store.low();
        let atr_m = ctx.store.get(Indicator::Atr14);

        let mut stops = vec![f32::NAN; nr * nc];

        for row in 0..nr {
            for &col in ctx.col_indices(&all_cols) {
                let col = col as usize;
                if !input.get(row, col) {
                    continue;
                }
                let close = close_m.get(row, col);
                let low = low_m.get(row, col);
                let atr = atr_m.get(row, col);

                let stop = if !atr.is_nan() && !low.is_nan() {
                    if (close - low) > atr { close - atr } else { low }
                } else if !low.is_nan() {
                    low
                } else {
                    close * (1.0 - fallback_pct)
                };

                stops[row * nc + col] = stop;
            }
        }

        Ok(Slot::Matrix(WideMatrix::new(stops, nr, nc)))
    }
}

// ---------------------------------------------------------------------------
// FixedPct
// ---------------------------------------------------------------------------

/// Simple percentage stop: `stop = close * (1 - pct)`.
///
/// Always computes the long-side stop. The simulator handles direction
/// inversion for short positions.
///
/// Config: `"pct"` (f32, e.g. 0.05 for a 5% stop).
pub struct FixedPct;

impl Block for FixedPct {
    fn name(&self) -> &'static str { "fixed_pct" }
    fn output_type(&self) -> SlotType { SlotType::Matrix }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let pct = get_f32_or(ctx.config, "pct", 0.05);
        let input = ctx.input_mask().ok_or_else(|| anyhow::anyhow!("fixed_pct: no input mask"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let all_cols: Vec<u32> = (0..nc as u32).collect();

        let close_m = ctx.store.close();
        let mut stops = vec![f32::NAN; nr * nc];

        for row in 0..nr {
            for &col in ctx.col_indices(&all_cols) {
                let col = col as usize;
                if !input.get(row, col) {
                    continue;
                }
                let close = close_m.get(row, col);
                if !close.is_nan() {
                    stops[row * nc + col] = close * (1.0 - pct);
                }
            }
        }

        Ok(Slot::Matrix(WideMatrix::new(stops, nr, nc)))
    }
}

// ---------------------------------------------------------------------------
// GapDayLow
// ---------------------------------------------------------------------------

/// EP-specific stop: the gap day's low price.
///
/// No config needed. For each cell in the input mask, stop = low.
pub struct GapDayLow;

impl Block for GapDayLow {
    fn name(&self) -> &'static str { "gap_day_low" }
    fn output_type(&self) -> SlotType { SlotType::Matrix }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let input = ctx.input_mask().ok_or_else(|| anyhow::anyhow!("gap_day_low: no input mask"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let all_cols: Vec<u32> = (0..nc as u32).collect();

        let low_m = ctx.store.low();
        let mut stops = vec![f32::NAN; nr * nc];

        for row in 0..nr {
            for &col in ctx.col_indices(&all_cols) {
                let col = col as usize;
                if !input.get(row, col) {
                    continue;
                }
                stops[row * nc + col] = low_m.get(row, col);
            }
        }

        Ok(Slot::Matrix(WideMatrix::new(stops, nr, nc)))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_helper_defaults() {
        let config = HashMap::new();
        assert!((get_f32_or(&config, "pct", 0.05) - 0.05).abs() < 1e-6);

        let mut config2 = HashMap::new();
        config2.insert("pct".into(), serde_json::json!(0.10));
        assert!((get_f32_or(&config2, "pct", 0.05) - 0.10).abs() < 1e-6);
    }

    #[test]
    fn block_names_and_types() {
        assert_eq!(AtrCappedLow.name(), "atr_capped_low");
        assert_eq!(AtrCappedLow.output_type(), SlotType::Matrix);

        assert_eq!(FixedPct.name(), "fixed_pct");
        assert_eq!(FixedPct.output_type(), SlotType::Matrix);

        assert_eq!(GapDayLow.name(), "gap_day_low");
        assert_eq!(GapDayLow.output_type(), SlotType::Matrix);
    }

    #[test]
    fn atr_capped_low_logic() {
        // Inline the same logic to verify correctness
        let close = 100.0_f32;
        let low = 95.0_f32;
        let atr = 6.0_f32;
        let fallback = 0.05_f32;

        // close - low = 5 < atr = 6 => stop = low
        let stop = if !atr.is_nan() && !low.is_nan() {
            if (close - low) > atr { close - atr } else { low }
        } else if !low.is_nan() {
            low
        } else {
            close * (1.0 - fallback)
        };
        assert!((stop - 95.0).abs() < 1e-6);

        // close - low = 10 > atr = 6 => stop = close - atr = 94
        let low2 = 90.0_f32;
        let stop2 = if !atr.is_nan() && !low2.is_nan() {
            if (close - low2) > atr { close - atr } else { low2 }
        } else {
            unreachable!()
        };
        assert!((stop2 - 94.0).abs() < 1e-6);
    }

    #[test]
    fn fixed_pct_formula() {
        let close = 200.0_f32;
        let pct = 0.05_f32;
        let stop = close * (1.0 - pct);
        assert!((stop - 190.0).abs() < 1e-6);
    }
}
