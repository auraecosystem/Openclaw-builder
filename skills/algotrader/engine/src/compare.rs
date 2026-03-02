//! Pipeline parity comparison: hardcoded vs dynamic strategies.
//!
//! Runs two strategies through the full filter → signals → simulate pipeline
//! and computes structural differences at each stage. Used by the parity smoke
//! test to ensure dynamic JSON configs produce equivalent results to their
//! hardcoded Rust counterparts.

use std::collections::HashMap;
use std::time::Instant;

use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use engine_data::{resolve_date_row, DataStore};
use engine_types::{Direction, Params, ResolvedParams, SignalSet, Trade, WideMask};

use crate::execution::{simulate, PositionSizer};
use crate::strategy::{create_setups, dynamic_strategy_paths};

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct CompareResult {
    pub hardcoded_name: String,
    pub dynamic_name: String,
    pub filter: FilterComparison,
    pub signals: SignalComparison,
    pub trades: TradeComparison,
    pub timing: TimingComparison,
}

#[derive(Debug, Serialize)]
pub struct FilterComparison {
    pub total_cells: usize,
    pub hardcoded_pass: usize,
    pub dynamic_pass: usize,
    /// Cells where both masks are true.
    pub both_pass: usize,
    /// Cells where either mask is true.
    pub either_pass: usize,
    /// Jaccard similarity: intersection / union.
    pub jaccard: f64,
    pub identical: bool,
    /// (row, col) of first 10 mismatches for debugging.
    pub mismatches: Vec<(usize, usize)>,
}

#[derive(Debug, Serialize)]
pub struct SignalComparison {
    pub entries_identical: bool,
    pub exits_identical: bool,
    pub entry_mismatches: usize,
    pub exit_mismatches: usize,
    pub stop_max_diff: f32,
    /// Stop prices differing by more than 1e-4.
    pub stop_mismatches: usize,
}

#[derive(Debug, Serialize)]
pub struct TradeComparison {
    pub hardcoded_count: usize,
    pub dynamic_count: usize,
    /// Trades with the same (entry_row, ticker_col, direction) key.
    pub matched_trades: usize,
    pub hardcoded_only: usize,
    pub dynamic_only: usize,
    pub pnl_hardcoded: f64,
    pub pnl_dynamic: f64,
    pub identical: bool,
}

#[derive(Debug, Serialize)]
pub struct TimingComparison {
    pub hardcoded_filter_ms: f64,
    pub dynamic_filter_ms: f64,
    pub hardcoded_signals_ms: f64,
    pub dynamic_signals_ms: f64,
    pub hardcoded_total_ms: f64,
    pub dynamic_total_ms: f64,
    /// dynamic / hardcoded for the filter phase.
    pub filter_ratio: f64,
    /// dynamic / hardcoded for the full pipeline.
    pub total_ratio: f64,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compare a hardcoded strategy against its dynamic JSON equivalent.
///
/// `hardcoded_name` is the key passed to `create_setups()` for the Rust
/// struct (e.g. `"ep"`). `dynamic_name` resolves to
/// `strategies/{dynamic_name}.json` (e.g. `"ep_dynamic"`).
///
/// When the hardcoded strategy produces multiple setups (e.g. `"breakout"`
/// returns two sub-strategies), only the first setup is used for comparison.
pub fn compare_strategies(
    store: &DataStore,
    params: &Params,
    hardcoded_name: &str,
    dynamic_name: &str,
) -> Result<CompareResult> {
    let nr = store.axes.n_rows;

    let start_row = params
        .start
        .as_ref()
        .and_then(|s| resolve_date_row(&store.axes, s))
        .unwrap_or(0);
    let end_row = params
        .end
        .as_ref()
        .and_then(|s| resolve_date_row(&store.axes, s))
        .unwrap_or(nr);

    let rp = ResolvedParams {
        params: params.clone(),
        start_row,
        end_row,
    };

    let hc_setups = create_setups(hardcoded_name);
    if hc_setups.is_empty() {
        anyhow::bail!("no setups found for hardcoded name: {hardcoded_name}");
    }
    // Use only the first setup when there are multiple (breakout → quick + runner).
    let hc = &hc_setups[0];

    // Load dynamic setup with param overrides from the Params struct so both
    // paths use identical thresholds (critical for --crypto or any CLI override).
    let overrides = params_to_overrides(params);
    let dyn_setups = load_dynamic_with_overrides(dynamic_name, &overrides)?;
    let dyn_setup = &dyn_setups[0];

    let sizer = PositionSizer::from_params(params);

    // --- Filter phase ---
    let t0 = Instant::now();
    let hc_filter = hc.filter(store, params, start_row..end_row);
    let hardcoded_filter_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let t0 = Instant::now();
    let dyn_filter = dyn_setup.filter(store, params, start_row..end_row);
    let dynamic_filter_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let filter = compare_masks(&hc_filter, &dyn_filter);

    // --- Signal phase (run both on the hardcoded filter to isolate differences) ---
    let t0 = Instant::now();
    let hc_signals = hc.signals(store, &hc_filter, params);
    let hardcoded_signals_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let t0 = Instant::now();
    let dyn_signals = dyn_setup.signals(store, &hc_filter, params);
    let dynamic_signals_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let signals = compare_signals(&hc_signals, &dyn_signals);

    // --- Simulation phase ---
    let t_hc_start = Instant::now();
    // Re-run signals on the correct filter masks for trade comparison.
    let hc_sigs_for_trades = hc.signals(store, &hc_filter, params);
    let hc_trades = simulate(
        store,
        &hc_sigs_for_trades,
        &rp,
        &sizer,
        &hc.exit_rules(params),
        hc.direction(),
        hc.equity_fraction(),
        hc.fill_mode(),
        hc.name(),
    );
    let hardcoded_total_ms = t_hc_start.elapsed().as_secs_f64() * 1000.0 + hardcoded_filter_ms;

    let t_dyn_start = Instant::now();
    let dyn_sigs_for_trades = dyn_setup.signals(store, &dyn_filter, params);
    let dyn_trades = simulate(
        store,
        &dyn_sigs_for_trades,
        &rp,
        &sizer,
        &dyn_setup.exit_rules(params),
        dyn_setup.direction(),
        dyn_setup.equity_fraction(),
        dyn_setup.fill_mode(),
        dyn_setup.name(),
    );
    let dynamic_total_ms = t_dyn_start.elapsed().as_secs_f64() * 1000.0 + dynamic_filter_ms;

    let trades = compare_trades(&hc_trades, &dyn_trades);

    let filter_ratio = if hardcoded_filter_ms > 0.0 {
        dynamic_filter_ms / hardcoded_filter_ms
    } else {
        1.0
    };
    let total_ratio = if hardcoded_total_ms > 0.0 {
        dynamic_total_ms / hardcoded_total_ms
    } else {
        1.0
    };

    Ok(CompareResult {
        hardcoded_name: hardcoded_name.to_string(),
        dynamic_name: dynamic_name.to_string(),
        filter,
        signals,
        trades,
        timing: TimingComparison {
            hardcoded_filter_ms,
            dynamic_filter_ms,
            hardcoded_signals_ms,
            dynamic_signals_ms,
            hardcoded_total_ms,
            dynamic_total_ms,
            filter_ratio,
            total_ratio,
        },
    })
}

/// Run all known hardcoded-vs-dynamic comparisons.
///
/// Pairs tested:
/// - `"ep"` vs `"ep_dynamic"`
/// - `"parabolic"` vs `"parabolic_dynamic"`
/// - `"breakout"` (first sub-strategy only) vs `"breakout_quick"`
///
/// Results are returned in the order listed above. Any pair whose strategies
/// cannot be loaded is skipped with an `eprintln!` warning rather than
/// aborting the whole run.
pub fn compare_all(store: &DataStore, params: &Params) -> Vec<CompareResult> {
    let pairs = [
        ("ep", "ep_dynamic"),
        ("parabolic", "parabolic_dynamic"),
        ("breakout", "breakout_quick"),
    ];

    pairs
        .iter()
        .filter_map(|(hc, dyn_name)| {
            match compare_strategies(store, params, hc, dyn_name) {
                Ok(result) => Some(result),
                Err(e) => {
                    eprintln!("compare_strategies({hc}, {dyn_name}) failed: {e}");
                    None
                }
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map `Params` struct fields to JSON param names for override injection.
fn params_to_overrides(params: &Params) -> HashMap<String, serde_json::Value> {
    let mut m = HashMap::new();
    // Shared universe filters
    m.insert("min_price".into(), json!(params.min_price));
    m.insert("min_vol".into(), json!(params.min_vol));
    m.insert("min_adv".into(), json!(params.min_adv));
    // Breakout params
    m.insert("rs_pct".into(), json!(params.rs_pct));
    m.insert("vol_ratio".into(), json!(params.vol_ratio));
    m.insert("max_range_pct".into(), json!(params.max_range_pct));
    m.insert("max_dist_52w".into(), json!(params.max_dist_52w));
    m.insert("min_prior_move".into(), json!(params.min_prior_move));
    m.insert("min_adr_pct".into(), json!(params.min_adr_pct));
    m.insert("min_consol_days".into(), json!(params.min_consol_days));
    m.insert("regime".into(), json!(params.regime));
    m.insert("stop_fallback_pct".into(), json!(params.strategy.stop_fallback_pct));
    // EP params
    m.insert("ep_min_gap_pct".into(), json!(params.strategy.ep_min_gap_pct));
    m.insert("ep_min_vol_ratio".into(), json!(params.strategy.ep_min_vol_ratio));
    m.insert("ep_min_gap_day_dollar_vol".into(), json!(params.strategy.ep_min_gap_day_dollar_vol));
    m.insert("ep_max_prior_6m_return".into(), json!(params.strategy.ep_max_prior_6m_return));
    // Parabolic params
    m.insert("large_cap_price".into(), json!(params.strategy.large_cap_price));
    m.insert("large_cap_run".into(), json!(params.strategy.large_cap_run));
    m.insert("small_cap_run".into(), json!(params.strategy.small_cap_run));
    m
}

/// Load a dynamic setup with param overrides, wrapping it in a Setup adapter.
fn load_dynamic_with_overrides(
    name: &str,
    overrides: &HashMap<String, serde_json::Value>,
) -> Result<Vec<Box<dyn crate::strategy::Setup>>> {
    use crate::strategy::DynamicSetupAdapter;

    let candidates = dynamic_strategy_paths(name);
    for path in &candidates {
        if path.exists() {
            let json_str = std::fs::read_to_string(path)?;
            let inner = engine_pipeline::DynamicSetup::from_json_with_overrides(&json_str, overrides)?;
            return Ok(vec![Box::new(DynamicSetupAdapter { inner })]);
        }
    }
    anyhow::bail!("no JSON config found for dynamic strategy: {name}");
}

fn compare_masks(a: &WideMask, b: &WideMask) -> FilterComparison {
    let total_cells = a.data.len();
    let mut hardcoded_pass = 0usize;
    let mut dynamic_pass = 0usize;
    let mut both_pass = 0usize;
    let mut either_pass = 0usize;
    let mut mismatches: Vec<(usize, usize)> = Vec::new();
    let identical_ref = std::cell::Cell::new(true);

    let nc = a.n_cols();

    for i in 0..total_cells {
        let av = a.data[i];
        let bv = b.data[i];

        if av { hardcoded_pass += 1; }
        if bv { dynamic_pass += 1; }
        if av && bv { both_pass += 1; }
        if av || bv { either_pass += 1; }

        if av != bv {
            identical_ref.set(false);
            if mismatches.len() < 10 {
                mismatches.push((i / nc, i % nc));
            }
        }
    }

    let jaccard = if either_pass == 0 {
        1.0 // both empty → identical
    } else {
        both_pass as f64 / either_pass as f64
    };

    FilterComparison {
        total_cells,
        hardcoded_pass,
        dynamic_pass,
        both_pass,
        either_pass,
        jaccard,
        identical: identical_ref.get(),
        mismatches,
    }
}

fn compare_signals(a: &SignalSet, b: &SignalSet) -> SignalComparison {
    let mut entry_mismatches = 0usize;
    let mut exit_mismatches = 0usize;
    let mut stop_max_diff = 0f32;
    let mut stop_mismatches = 0usize;

    for (av, bv) in a.entries.data.iter().zip(b.entries.data.iter()) {
        if av != bv {
            entry_mismatches += 1;
        }
    }

    for (av, bv) in a.exits.data.iter().zip(b.exits.data.iter()) {
        if av != bv {
            exit_mismatches += 1;
        }
    }

    for (av, bv) in a.stop_prices.data.iter().zip(b.stop_prices.data.iter()) {
        // Ignore NaN vs NaN (treated as matching) and skip NaN comparisons.
        if av.is_nan() && bv.is_nan() {
            continue;
        }
        let diff = (av - bv).abs();
        if diff > stop_max_diff {
            stop_max_diff = diff;
        }
        if diff > 1e-4 {
            stop_mismatches += 1;
        }
    }

    SignalComparison {
        entries_identical: entry_mismatches == 0,
        exits_identical: exit_mismatches == 0,
        entry_mismatches,
        exit_mismatches,
        stop_max_diff,
        stop_mismatches,
    }
}

fn compare_trades(a: &[Trade], b: &[Trade]) -> TradeComparison {
    use std::collections::HashSet;

    // Match key: (entry_row, ticker_col, direction as u8)
    fn key(t: &Trade) -> (usize, usize, u8) {
        let dir = match t.direction {
            Direction::Long => 0,
            Direction::Short => 1,
        };
        (t.entry_row, t.ticker_col, dir)
    }

    let a_keys: HashSet<_> = a.iter().map(key).collect();
    let b_keys: HashSet<_> = b.iter().map(key).collect();

    let matched_trades = a_keys.intersection(&b_keys).count();
    let hardcoded_only = a_keys.difference(&b_keys).count();
    let dynamic_only = b_keys.difference(&a_keys).count();

    let pnl_hardcoded: f64 = a.iter().map(|t| t.pnl).sum();
    let pnl_dynamic: f64 = b.iter().map(|t| t.pnl).sum();

    let identical = hardcoded_only == 0 && dynamic_only == 0 && a.len() == b.len();

    TradeComparison {
        hardcoded_count: a.len(),
        dynamic_count: b.len(),
        matched_trades,
        hardcoded_only,
        dynamic_only,
        pnl_hardcoded,
        pnl_dynamic,
        identical,
    }
}
