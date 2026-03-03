//! Entry condition blocks.
//!
//! Entry blocks consume a universe `WideMask` and produce a `SignalSet`
//! containing entry mask, exit mask, and stop prices. The exit mask is
//! typically close < SMA10, applied across all cells (not just the universe).

use std::collections::HashMap;

use engine_data::Indicator;
use engine_types::{SignalSet, WideMask, WideMatrix};

use crate::blackboard::{Slot, SlotType};
use super::BlockContext;
use super::Block;

// ---------------------------------------------------------------------------
// Config helpers (same signatures as pattern.rs -- small duplication is fine
// to keep each block file self-contained)
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
// Shared: ATR-capped stop (inlined from strategy/mod.rs:atr_stop)
// ---------------------------------------------------------------------------

/// ATR-capped stop: low-of-day, but no more than 1x ATR below close.
/// Falls back to `close * (1 - fallback_pct)` when ATR or low is unavailable.
#[inline]
fn atr_stop(close: f32, low: f32, atr: f32, fallback_pct: f32) -> f32 {
    if !atr.is_nan() && !low.is_nan() {
        if (close - low) > atr { close - atr } else { low }
    } else if !low.is_nan() {
        low
    } else {
        close * (1.0 - fallback_pct)
    }
}

/// Populate the exit mask: close < SMA10 everywhere (independent of universe).
fn fill_sma10_exits(store: &engine_data::DataStore) -> WideMask {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;
    let close_m = store.close();
    let sma10 = store.get(Indicator::Sma10);
    let mut exits = WideMask::new_false(nr, nc);

    for row in 0..nr {
        for col in 0..nc {
            let close = close_m.get(row, col);
            let sma = sma10.get(row, col);
            if !close.is_nan() && !sma.is_nan() && close < sma {
                exits.set(row, col, true);
            }
        }
    }
    exits
}

// ---------------------------------------------------------------------------
// BreakoutAbove
// ---------------------------------------------------------------------------

/// Entry when close breaks above a reference indicator with a volume spike.
///
/// Produces entries where close > reference level AND volume exceeds the
/// spike threshold times the 20-day volume SMA. Stop prices use the
/// ATR-capped-low formula. Exit mask is close < SMA10 everywhere.
///
/// Config:
/// - `ref`: indicator name for reference level (default "consol_high")
/// - `vol_spike`: volume ratio threshold (default 1.5)
/// - `adv_min`: minimum dollar volume (default 0.0, disabled)
/// - `stop_fallback_pct`: fallback percentage stop (default 0.05)
pub struct BreakoutAbove;

impl Block for BreakoutAbove {
    fn name(&self) -> &'static str { "breakout_above" }
    fn output_type(&self) -> SlotType { SlotType::Signals }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let vol_spike = get_f32_or(ctx.config, "vol_spike", 1.5);
        let adv_min = get_f32_or(ctx.config, "adv_min", 0.0);
        let stop_fallback = get_f32_or(ctx.config, "stop_fallback_pct", 0.05);

        // Resolve reference indicator (default: ConsolHigh)
        let ref_name = ctx
            .config
            .get("ref")
            .and_then(|v| v.as_str())
            .unwrap_or("consol_high");
        let ref_ind = match ref_name {
            "consol_high" => Indicator::ConsolHigh,
            "sma10" => Indicator::Sma10,
            "sma20" => Indicator::Sma20,
            other => anyhow::bail!("breakout_above: unknown ref indicator '{other}'"),
        };

        let input = ctx.input_mask().ok_or_else(|| anyhow::anyhow!("breakout_above: no input mask"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let all_cols: Vec<u32> = (0..nc as u32).collect();

        let close_m = ctx.store.close();
        let vol_m = ctx.store.volume();
        let vol_sma = ctx.store.get(Indicator::VolSma20);
        let ref_m = ctx.store.get(ref_ind);
        let low_m = ctx.store.low();
        let atr_m = ctx.store.get(Indicator::Atr14);

        let mut entries = WideMask::new_false(nr, nc);
        let mut stops = vec![f32::NAN; nr * nc];

        for row in 0..nr {
            for &col in ctx.col_indices(&all_cols) {
                let col = col as usize;
                if !input.get(row, col) {
                    continue;
                }
                let close = close_m.get(row, col);
                let ref_val = ref_m.get(row, col);

                // Breakout: close must exceed the reference level
                if ref_val.is_nan() || close <= ref_val {
                    continue;
                }

                // Volume spike check
                let vol = vol_m.get(row, col);
                let vsma = vol_sma.get(row, col);
                if vol <= vol_spike * vsma {
                    continue;
                }

                // ADV filter (skip if adv_min is zero / disabled)
                if adv_min > 0.0 && vsma * close < adv_min {
                    continue;
                }

                let i = row * nc + col;
                entries.data[i] = true;
                stops[i] = atr_stop(
                    close,
                    low_m.get(row, col),
                    atr_m.get(row, col),
                    stop_fallback,
                );
            }
        }

        let exits = fill_sma10_exits(ctx.store);
        let stop_prices = WideMatrix::new(stops, nr, nc);

        Ok(Slot::Signals(SignalSet { entries, exits, stop_prices }))
    }
}

// ---------------------------------------------------------------------------
// GapEntry
// ---------------------------------------------------------------------------

/// EP-style gap entry: shifts input mask forward by 1 row (entry on the day
/// after the gap), with stop = gap day's low.
///
/// The hardcoded EP strategy enters at row+1 so the fill happens on the open
/// of the day after the gap. The stop is the low of the gap day (row), not
/// the entry day (row+1).
pub struct GapEntry;

impl Block for GapEntry {
    fn name(&self) -> &'static str { "gap_entry" }
    fn output_type(&self) -> SlotType { SlotType::Signals }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let input = ctx.input_mask().ok_or_else(|| anyhow::anyhow!("gap_entry: no input mask"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let all_cols: Vec<u32> = (0..nc as u32).collect();

        let low_m = ctx.store.low();

        let mut entries = WideMask::new_false(nr, nc);
        let mut stops = vec![f32::NAN; nr * nc];

        // Shift entry to row+1; stop = gap day's (row) low.
        for row in 0..nr {
            for &col in ctx.col_indices(&all_cols) {
                let col = col as usize;
                if !input.get(row, col) {
                    continue;
                }
                if row + 1 >= nr {
                    continue; // no room to shift
                }
                let i = (row + 1) * nc + col;
                entries.data[i] = true;
                stops[i] = low_m.get(row, col); // stop from gap day
            }
        }

        let exits = fill_sma10_exits(ctx.store);
        let stop_prices = WideMatrix::new(stops, nr, nc);

        Ok(Slot::Signals(SignalSet { entries, exits, stop_prices }))
    }
}

// ---------------------------------------------------------------------------
// ScoreThreshold
// ---------------------------------------------------------------------------

/// Entry when a score matrix (from a prior pipeline step) exceeds a threshold.
///
/// Reads a `Matrix` slot from the blackboard by `score_input` ID, thresholds
/// it to produce the entry mask, then applies ATR-capped stops.
///
/// Config:
/// - `score_input`: step ID of the score WideMatrix on the blackboard
/// - `threshold`: f32 score threshold
/// - `stop_fallback_pct`: fallback percentage stop (default 0.05)
pub struct ScoreThreshold;

impl Block for ScoreThreshold {
    fn name(&self) -> &'static str { "score_threshold" }
    fn output_type(&self) -> SlotType { SlotType::Signals }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let score_id = ctx
            .config
            .get("score_input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("score_threshold: missing 'score_input'"))?;
        let threshold = get_f32_or(ctx.config, "threshold", 0.5);
        let stop_fallback = get_f32_or(ctx.config, "stop_fallback_pct", 0.05);

        let score_slot = ctx
            .blackboard
            .get(score_id)
            .ok_or_else(|| anyhow::anyhow!("score_threshold: '{score_id}' not on blackboard"))?;
        let score_mat = score_slot
            .as_matrix()
            .ok_or_else(|| anyhow::anyhow!("score_threshold: '{score_id}' is not a Matrix"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let all_cols: Vec<u32> = (0..nc as u32).collect();

        let close_m = ctx.store.close();
        let low_m = ctx.store.low();
        let atr_m = ctx.store.get(Indicator::Atr14);

        let mut entries = WideMask::new_false(nr, nc);
        let mut stops = vec![f32::NAN; nr * nc];

        for row in 0..nr {
            for &col in ctx.col_indices(&all_cols) {
                let col = col as usize;
                let score = score_mat.get(row, col);
                if score.is_nan() || score < threshold {
                    continue;
                }
                let i = row * nc + col;
                entries.data[i] = true;

                let close = close_m.get(row, col);
                stops[i] = atr_stop(
                    close,
                    low_m.get(row, col),
                    atr_m.get(row, col),
                    stop_fallback,
                );
            }
        }

        let exits = fill_sma10_exits(ctx.store);
        let stop_prices = WideMatrix::new(stops, nr, nc);

        Ok(Slot::Signals(SignalSet { entries, exits, stop_prices }))
    }
}

// ---------------------------------------------------------------------------
// ParabolicEntry
// ---------------------------------------------------------------------------

/// Parabolic short entry: input mask is the entry mask, stop = entry day's
/// high, exit = close <= SMA10 OR close <= SMA20 (mean reversion cover).
///
/// Unlike other entry blocks that use fill_sma10_exits(), this uses a dual-SMA
/// OR exit to match the hardcoded ParabolicShort strategy.
pub struct ParabolicEntry;

impl Block for ParabolicEntry {
    fn name(&self) -> &'static str { "parabolic_entry" }
    fn output_type(&self) -> SlotType { SlotType::Signals }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let input = ctx.input_mask().ok_or_else(|| anyhow::anyhow!("parabolic_entry: no input mask"))?;

        let nr = ctx.store.axes.n_rows;
        let nc = ctx.store.axes.n_cols;
        let all_cols: Vec<u32> = (0..nc as u32).collect();

        let close_m = ctx.store.close();
        let high_m = ctx.store.high();
        let sma10 = ctx.store.get(Indicator::Sma10);
        let sma20 = ctx.store.get(Indicator::Sma20);

        let mut entries = WideMask::new_false(nr, nc);
        let mut exits = WideMask::new_false(nr, nc);
        let mut stops = vec![f32::NAN; nr * nc];

        for row in 0..nr {
            for &col in ctx.col_indices(&all_cols) {
                let col = col as usize;
                let i = row * nc + col;
                let close = close_m.get(row, col);
                let s10 = sma10.get(row, col);
                let s20 = sma20.get(row, col);

                // Cover when price reverts to either SMA
                if !close.is_nan()
                    && ((!s10.is_nan() && close <= s10) || (!s20.is_nan() && close <= s20))
                {
                    exits.data[i] = true;
                }

                if !input.get(row, col) {
                    continue;
                }

                entries.data[i] = true;
                // Stop: entry day's high (if price reclaims, short thesis is wrong)
                stops[i] = high_m.get(row, col);
            }
        }

        let stop_prices = WideMatrix::new(stops, nr, nc);
        Ok(Slot::Signals(SignalSet { entries, exits, stop_prices }))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atr_stop_normal() {
        // close=100, low=95, atr=6 => close - low (5) < atr (6) => stop = low
        assert!((atr_stop(100.0, 95.0, 6.0, 0.05) - 95.0).abs() < 1e-6);
    }

    #[test]
    fn atr_stop_capped() {
        // close=100, low=90, atr=6 => close - low (10) > atr (6) => stop = close - atr = 94
        assert!((atr_stop(100.0, 90.0, 6.0, 0.05) - 94.0).abs() < 1e-6);
    }

    #[test]
    fn atr_stop_nan_atr_uses_low() {
        assert!((atr_stop(100.0, 95.0, f32::NAN, 0.05) - 95.0).abs() < 1e-6);
    }

    #[test]
    fn atr_stop_nan_both_uses_fallback() {
        let result = atr_stop(100.0, f32::NAN, f32::NAN, 0.05);
        assert!((result - 95.0).abs() < 1e-6);
    }

    #[test]
    fn config_helper_defaults() {
        let config = HashMap::new();
        assert!((get_f32_or(&config, "x", 1.5) - 1.5).abs() < 1e-6);
    }

    #[test]
    fn block_names_and_types() {
        assert_eq!(BreakoutAbove.name(), "breakout_above");
        assert_eq!(BreakoutAbove.output_type(), SlotType::Signals);

        assert_eq!(GapEntry.name(), "gap_entry");
        assert_eq!(GapEntry.output_type(), SlotType::Signals);

        assert_eq!(ScoreThreshold.name(), "score_threshold");
        assert_eq!(ScoreThreshold.output_type(), SlotType::Signals);

        assert_eq!(ParabolicEntry.name(), "parabolic_entry");
        assert_eq!(ParabolicEntry.output_type(), SlotType::Signals);
    }
}
