//! Algotrader engine crate root.
//!
//! Exposes the core backtest pipeline: load data, configure parameters, run
//! single or batch backtests. The binary (`main.rs`) is a thin CLI dispatcher
//! that calls into this library.

pub mod analysis;
pub mod data;
pub mod evolution;
pub mod execution;
pub mod server;
pub mod signals;
pub mod strategy;
pub mod types;
pub mod walk_forward;

use std::fs;
use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;

use analysis::Report;
use data::{resolve_date_row, DataStore};
use execution::PositionSizer;
use strategy::create_setups;
use types::{Params, ResolvedParams};

pub use data::{load_data_store, Axes};

/// Run a single backtest: resolve dates, create setups, filter/signal/simulate
/// for each sub-strategy, merge trades, produce a report.
pub fn run_single(store: &DataStore, params: &Params) -> Report {
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
        return analysis::generate_report(&[], &store.axes, params, 1);
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
            &setup.exit_rules(),
            setup.direction(),
            setup.equity_fraction(),
            setup.fill_mode(),
            setup.name(),
        );
        all_trades.extend(trades);
    }

    analysis::generate_report(&all_trades, &store.axes, params, 1)
}

/// Like `run_single()` but also returns the raw trades for CSV export.
pub fn run_single_with_trades(store: &DataStore, params: &Params) -> (Report, Vec<types::Trade>) {
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
        return (analysis::generate_report(&[], &store.axes, params, 1), Vec::new());
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
            &setup.exit_rules(),
            setup.direction(),
            setup.equity_fraction(),
            setup.fill_mode(),
            setup.name(),
        );
        all_trades.extend(trades);
    }

    let report = analysis::generate_report(&all_trades, &store.axes, params, 1);
    (report, all_trades)
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
        .map(|params| run_batch_single(store, params, n_trials))
        .collect();

    Ok(reports)
}

/// Single backtest within a batch — passes n_trials for DSR correction.
fn run_batch_single(store: &DataStore, params: &Params, n_trials: usize) -> Report {
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
        return analysis::generate_report(&[], &store.axes, params, n_trials);
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
            &setup.exit_rules(),
            setup.direction(),
            setup.equity_fraction(),
            setup.fill_mode(),
            setup.name(),
        );
        all_trades.extend(trades);
    }

    analysis::generate_report(&all_trades, &store.axes, params, n_trials)
}
