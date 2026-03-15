//! Algotrader engine crate root.
//!
//! Exposes the core backtest pipeline: load data, configure parameters, run
//! single or batch backtests. The binary (`main.rs`) is a thin CLI dispatcher
//! that calls into this library.

pub mod analysis;
pub mod compare;
pub mod evolution;
pub mod execution;
pub mod portfolio;
#[cfg(feature = "python")]
pub mod python;
pub mod server;
pub mod strategy;
pub mod trading_config;
pub mod walk_forward;

// Re-export sub-crates so existing `algotrader_engine::types::X`,
// `algotrader_engine::data::X`, and `algotrader_engine::signals::X`
// paths continue to resolve (tests, main.rs, cli.rs all rely on this).
pub use engine_types as types;
pub use engine_data as data;
pub use engine_signals as signals;

use std::fs;
use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;

use analysis::Report;
use engine_data::{resolve_date_row, DataStore};
use execution::PositionSizer;
use strategy::create_setups;
use engine_types::{Params, ResolvedParams};

pub use engine_data::{load_data_store, Axes};

/// Core backtest loop: resolve dates, run all setups, return merged trades.
pub(crate) fn run_core(store: &DataStore, params: &Params) -> (Vec<engine_types::Trade>, usize, usize) {
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

    let setups = create_setups(&params.setup);
    if setups.is_empty() {
        eprintln!("  Unknown setup: {}", params.setup);
        return (Vec::new(), start_row, end_row);
    }

    let sizer = PositionSizer::from_params(params);
    let mut all_trades = Vec::new();

    for setup in &setups {
        let universe = setup.filter(store, params, start_row..end_row);
        let signals = setup.signals(store, &universe, params);
        let trades = execution::simulate(
            store,
            &signals,
            &rp,
            &sizer,
            &setup.exit_rules(params),
            setup.direction(),
            setup.equity_fraction(),
            setup.fill_mode(),
            setup.name(),
        );
        all_trades.extend(trades);
    }

    (all_trades, start_row, end_row)
}

/// Run a single backtest: resolve dates, create setups, filter/signal/simulate
/// for each sub-strategy, merge trades, produce a report.
pub fn run_single(store: &DataStore, params: &Params) -> Report {
    let (trades, _, _) = run_core(store, params);
    analysis::generate_report(&trades, &store.axes, params, 1)
}

/// Run a single backtest with pipeline param overrides for dynamic JSON strategies.
///
/// Re-creates the DynamicSetup with the given overrides applied before `@param`
/// interpolation. Used by the evolution loop to inject mutated parameters.
pub fn run_single_dynamic(
    store: &DataStore,
    params: &Params,
    overrides: &std::collections::HashMap<String, serde_json::Value>,
) -> Report {
    let (trades, _, _) = run_core_dynamic(store, params, overrides);
    analysis::generate_report(&trades, &store.axes, params, 1)
}

/// Core backtest loop with pipeline param overrides.
fn run_core_dynamic(
    store: &DataStore,
    params: &Params,
    overrides: &std::collections::HashMap<String, serde_json::Value>,
) -> (Vec<engine_types::Trade>, usize, usize) {
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

    let setups = strategy::create_setups_with_overrides(&params.setup, overrides);
    if setups.is_empty() {
        eprintln!("  Unknown setup: {}", params.setup);
        return (Vec::new(), start_row, end_row);
    }

    let sizer = PositionSizer::from_params(params);
    let mut all_trades = Vec::new();

    for setup in &setups {
        let universe = setup.filter(store, params, start_row..end_row);
        let signals = setup.signals(store, &universe, params);
        let trades = execution::simulate(
            store,
            &signals,
            &rp,
            &sizer,
            &setup.exit_rules(params),
            setup.direction(),
            setup.equity_fraction(),
            setup.fill_mode(),
            setup.name(),
        );
        all_trades.extend(trades);
    }

    (all_trades, start_row, end_row)
}

/// Like `run_single()` but also returns the raw trades for CSV export.
pub fn run_single_with_trades(store: &DataStore, params: &Params) -> (Report, Vec<engine_types::Trade>) {
    let (trades, _, _) = run_core(store, params);
    let report = analysis::generate_report(&trades, &store.axes, params, 1);
    (report, trades)
}

/// Run a batch of backtests with different parameter sets, sharing one DataStore.
/// Reads a JSON array of `Params` from the given file and runs them in parallel.
/// Each report's DSR accounts for the total number of parameter sets tested.
pub fn run_batch(store: &DataStore, params_file: &Path) -> Result<Vec<Report>> {
    let content = fs::read_to_string(params_file)?;
    let param_sets: Vec<Params> = serde_json::from_str(&content)?;

    let n_trials = param_sets.len();
    eprintln!("  Batch: {} parameter sets", n_trials);

    let reports: Vec<Report> = param_sets
        .par_iter()
        .map(|params| {
            let (trades, _, _) = run_core(store, params);
            analysis::generate_report(&trades, &store.axes, params, n_trials)
        })
        .collect();

    Ok(reports)
}
