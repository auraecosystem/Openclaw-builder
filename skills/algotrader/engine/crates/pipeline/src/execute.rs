//! Execute a compiled physical plan against a DataStore.
//!
//! Two entry points:
//! - `execute_filter_phase` runs filter steps (fused + single) to produce a WideMask.
//! - `execute_signal_phase` runs the signal step to produce a SignalSet.
//!
//! Both operate on a `Blackboard` for intermediate storage, seeded with an
//! all-true mask representing the full universe.

use std::collections::HashMap;
use std::ops::Range;

use engine_data::DataStore;
use engine_types::{SignalSet, WideMask, WideMatrix};

use crate::blackboard::{Blackboard, Slot};
use crate::blocks::BlockContext;
use crate::plan::PhysicalStep;
use crate::registry::BlockRegistry;

// ---------------------------------------------------------------------------
// Filter phase
// ---------------------------------------------------------------------------

/// Create an all-true mask covering the given dimensions.
fn all_true_mask(n_rows: usize, n_cols: usize) -> WideMask {
    let mut mask = WideMask::new_false(n_rows, n_cols);
    for i in 0..mask.data.len() {
        mask.data[i] = true;
    }
    mask
}

/// Check whether a `when` condition evaluates to true.
///
/// Looks up the config key in the step's config map. A missing key or a
/// non-boolean value is treated as true (step runs by default). An explicit
/// `false` disables the step.
fn when_enabled(config: &HashMap<String, serde_json::Value>, when_key: &str) -> bool {
    match config.get(when_key) {
        Some(v) => v.as_bool().unwrap_or(true),
        None => true,
    }
}

/// Execute the filter phase of a physical plan.
///
/// Seeds a blackboard with an all-true mask, then runs each step in order.
/// Fused steps apply their conditions in a single vectorized pass. Single steps
/// dispatch to the block registry. Returns the final mask after all filters.
pub fn execute_filter_phase(
    steps: &[PhysicalStep],
    store: &DataStore,
    registry: &BlockRegistry,
    range: Range<usize>,
) -> anyhow::Result<WideMask> {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;
    let mut bb = Blackboard::new();

    // Seed with full universe.
    let seed = all_true_mask(nr, nc);
    bb.put("__seed".to_string(), Slot::Mask(seed));

    for step in steps {
        match step {
            PhysicalStep::Fused(fused) => {
                let input = bb
                    .last()
                    .and_then(|s| s.as_mask())
                    .ok_or_else(|| anyhow::anyhow!("fused step: no input mask on blackboard"))?;
                let result = fused.execute(store, input, range.clone());
                bb.put(fused.step_id.clone(), Slot::Mask(result));
            }
            PhysicalStep::Single {
                block_name,
                config,
                step_id,
                input_id,
                when,
            } => {
                // Check conditional gate.
                if let Some(when_key) = when {
                    if !when_enabled(config, when_key) {
                        // Skip: carry forward previous mask under this step's ID.
                        if let Some(prev) = bb.last().and_then(|s| s.as_mask()) {
                            bb.put(step_id.clone(), Slot::Mask(prev.clone()));
                        }
                        continue;
                    }
                }

                let block = registry
                    .get(block_name)
                    .ok_or_else(|| anyhow::anyhow!("unknown block: {block_name}"))?;

                let ctx = BlockContext {
                    store,
                    config,
                    range: range.clone(),
                    blackboard: &bb,
                    input_id: input_id.as_deref(),
                };

                let result = block.execute(&ctx)?;
                bb.put(step_id.clone(), result);
            }
        }
    }

    // Return the final mask from the blackboard.
    bb.last()
        .and_then(|s| s.as_mask())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("filter phase produced no mask"))
}

// ---------------------------------------------------------------------------
// Signal phase
// ---------------------------------------------------------------------------

/// Execute the signal phase of a physical plan.
///
/// If a signal step exists, creates a blackboard seeded with the universe mask,
/// dispatches to the signal block, and extracts a `SignalSet`. If no signal
/// step is configured, returns an empty `SignalSet` (no entries).
pub fn execute_signal_phase(
    step: Option<&PhysicalStep>,
    store: &DataStore,
    registry: &BlockRegistry,
    universe: &WideMask,
) -> anyhow::Result<SignalSet> {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;

    let step = match step {
        Some(s) => s,
        None => {
            // No signal step: return empty signals.
            return Ok(SignalSet {
                entries: WideMask::new_false(nr, nc),
                exits: WideMask::new_false(nr, nc),
                stop_prices: WideMatrix::new(vec![f32::NAN; nr * nc], nr, nc),
            });
        }
    };

    // Signal steps are always Single dispatches (never fused).
    let (block_name, config) = match step {
        PhysicalStep::Single {
            block_name, config, ..
        } => (block_name, config),
        PhysicalStep::Fused(_) => {
            anyhow::bail!("signal step must be a Single dispatch, not Fused");
        }
    };

    let block = registry
        .get(block_name)
        .ok_or_else(|| anyhow::anyhow!("unknown signal block: {block_name}"))?;

    let mut bb = Blackboard::new();
    bb.put("universe".to_string(), Slot::Mask(universe.clone()));

    let ctx = BlockContext {
        store,
        config,
        range: 0..nr,
        blackboard: &bb,
        input_id: None,
    };

    let result = block.execute(&ctx)?;
    match result {
        Slot::Signals(ss) => Ok(ss),
        _ => anyhow::bail!(
            "signal block '{block_name}' returned {:?}, expected Signals",
            result.slot_type()
        ),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuse::{FusedCondition, FusedFilter};
    use engine_data::{Axes, Indicator};
    use engine_types::WideMatrix;

    const INDICATOR_COUNT: usize = 26;

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
        let axes = Axes {
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

    #[test]
    fn empty_steps_returns_all_true() {
        let store = make_store(2, 3, &[]);
        let registry = BlockRegistry::new();
        let mask = execute_filter_phase(&[], &store, &registry, 0..2).unwrap();
        for row in 0..2 {
            for col in 0..3 {
                assert!(mask.get(row, col), "expected true at ({row},{col})");
            }
        }
    }

    #[test]
    fn single_block_step_filters() {
        // Use a PriceFloor block: close > 5.0 passes.
        let close_data = vec![1.0, 10.0, 3.0, 20.0, 4.0, 8.0];
        let store = make_store(2, 3, &[(Indicator::Close, close_data)]);

        let mut registry = BlockRegistry::new();
        registry.register(Box::new(
            crate::blocks::universe::PriceFloor,
        ));

        let mut config = HashMap::new();
        config.insert("min".into(), serde_json::json!(5.0));

        let steps = vec![PhysicalStep::Single {
            block_name: "price_floor".to_string(),
            config,
            step_id: "step_0".to_string(),
            input_id: None,
            when: None,
        }];

        let mask = execute_filter_phase(&steps, &store, &registry, 0..2).unwrap();
        assert!(!mask.get(0, 0)); // 1.0
        assert!(mask.get(0, 1));  // 10.0
        assert!(!mask.get(0, 2)); // 3.0
        assert!(mask.get(1, 0));  // 20.0
        assert!(!mask.get(1, 1)); // 4.0
        assert!(mask.get(1, 2));  // 8.0
    }

    #[test]
    fn fused_step_filters() {
        // Fuse: close > 5.0 AND volume > 100.0
        let close_data = vec![10.0, 3.0, 20.0, 1.0];
        let vol_data = vec![200.0, 200.0, 50.0, 300.0];
        let store = make_store(
            2,
            2,
            &[
                (Indicator::Close, close_data),
                (Indicator::Volume, vol_data),
            ],
        );

        let fused = FusedFilter {
            conditions: vec![
                FusedCondition {
                    indicator: Indicator::Close,
                    op: crate::blocks::CmpOp::Gt,
                    threshold: 5.0,
                },
                FusedCondition {
                    indicator: Indicator::Volume,
                    op: crate::blocks::CmpOp::Gt,
                    threshold: 100.0,
                },
            ],
            step_id: "fused_0".into(),
        };

        let steps = vec![PhysicalStep::Fused(fused)];
        let registry = BlockRegistry::new();
        let mask = execute_filter_phase(&steps, &store, &registry, 0..2).unwrap();

        assert!(mask.get(0, 0));   // close=10>5, vol=200>100
        assert!(!mask.get(0, 1));  // close=3, fails
        assert!(!mask.get(1, 0));  // close=20>5, vol=50<=100
        assert!(!mask.get(1, 1));  // close=1, fails
    }

    #[test]
    fn when_false_skips_step() {
        let close_data = vec![10.0, 20.0];
        let store = make_store(1, 2, &[(Indicator::Close, close_data)]);

        let mut registry = BlockRegistry::new();
        registry.register(Box::new(
            crate::blocks::universe::PriceFloor,
        ));

        // Set the "when" config key to false so the step is skipped.
        let mut config = HashMap::new();
        config.insert("min".into(), serde_json::json!(15.0));
        config.insert("regime".into(), serde_json::json!(false));

        let steps = vec![PhysicalStep::Single {
            block_name: "price_floor".to_string(),
            config,
            step_id: "step_0".to_string(),
            input_id: None,
            when: Some("regime".to_string()),
        }];

        let mask = execute_filter_phase(&steps, &store, &registry, 0..1).unwrap();
        // Step was skipped, so both cells should still be true (from seed).
        assert!(mask.get(0, 0));
        assert!(mask.get(0, 1));
    }

    #[test]
    fn signal_phase_no_step_returns_empty() {
        let store = make_store(2, 2, &[]);
        let registry = BlockRegistry::new();
        let universe = all_true_mask(2, 2);
        let signals = execute_signal_phase(None, &store, &registry, &universe).unwrap();
        // No entries.
        for row in 0..2 {
            for col in 0..2 {
                assert!(!signals.entries.get(row, col));
                assert!(!signals.exits.get(row, col));
                assert!(signals.stop_prices.get(row, col).is_nan());
            }
        }
    }
}
