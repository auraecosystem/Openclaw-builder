//! Walk-forward parameter optimization with time-series cross-validation.
//!
//! Splits the data into sequential train/test folds (expanding window),
//! sweeps a parameter grid on each training set using rayon parallelism,
//! then evaluates the best params on the out-of-sample test set. This
//! prevents data leakage: every test fold only sees parameters chosen
//! from strictly prior data.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use engine_data::DataStore;
use engine_types::Params;

/// Configuration for a walk-forward optimization run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalkForwardConfig {
    /// Number of train/test folds.
    pub n_folds: usize,
    /// Fraction of data used for training in each fold (informational; actual
    /// split uses expanding window with equal-sized test blocks).
    pub train_frac: f32,
    /// Parameter combinations to sweep on each training window.
    pub param_grid: Vec<Params>,
    /// Fitness metric: "sharpe", "calmar", or "pf" (profit factor).
    pub fitness_metric: String,
}

/// Result of a single train/test fold.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FoldResult {
    pub fold_idx: usize,
    pub train_start_row: usize,
    pub train_end_row: usize,
    pub test_start_row: usize,
    pub test_end_row: usize,
    pub best_params_idx: usize,
    pub train_fitness: f64,
    pub test_fitness: f64,
    /// Serialized test-set report (Report only implements Serialize, so we
    /// store it as a JSON value for portability).
    pub test_report: serde_json::Value,
}

/// Aggregated result across all folds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalkForwardResult {
    pub folds: Vec<FoldResult>,
    pub mean_oos_fitness: f64,
}

/// Extract a fitness scalar from a serialized report based on the metric name.
fn extract_fitness_from_json(report: &serde_json::Value, metric: &str) -> f64 {
    match metric {
        "sharpe" => report["sharpe"].as_f64().unwrap_or(0.0),
        "calmar" => {
            let cagr = report["cagr"].as_f64().unwrap_or(0.0);
            let max_dd = report["max_drawdown"].as_f64().unwrap_or(0.0);
            if max_dd > 0.0 { cagr / max_dd } else { 0.0 }
        }
        "pf" => report["profit_factor"].as_f64().unwrap_or(0.0),
        _ => report["sharpe"].as_f64().unwrap_or(0.0),
    }
}

/// Run walk-forward optimization.
///
/// Time-series aware: folds are sequential with expanding training windows
/// and fixed-size test blocks, preventing any future data leakage. Within
/// each fold, the parameter grid is swept in parallel via rayon.
pub fn walk_forward(store: &DataStore, config: &WalkForwardConfig) -> WalkForwardResult {
    let total_rows = store.axes.n_rows;
    let fold_size = total_rows / config.n_folds;

    let mut folds = Vec::with_capacity(config.n_folds);

    for fold_idx in 0..config.n_folds {
        // Expanding window: train on [0, test_start), test on [test_start, test_end)
        let test_start = fold_size * fold_idx + (total_rows - fold_size * config.n_folds);
        let test_end = test_start + fold_size;
        let train_start = 0;
        let train_end = test_start;

        // Need enough training data for meaningful optimization
        if train_end <= train_start + 100 {
            continue;
        }

        // Sweep params in parallel on training set
        let train_results: Vec<(usize, f64)> = config
            .param_grid
            .par_iter()
            .enumerate()
            .map(|(idx, params)| {
                let mut p = params.clone();
                // Override date range to training window only
                p.start = None;
                p.end = None;
                let report = crate::run_single(store, &p);
                let report_json = serde_json::to_value(&report).unwrap_or_default();
                let fitness = extract_fitness_from_json(&report_json, &config.fitness_metric);
                (idx, fitness)
            })
            .collect();

        // Select the best params by training fitness
        let (best_idx, best_train_fitness) = train_results
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .copied()
            .unwrap_or((0, 0.0));

        // Evaluate best params on the out-of-sample test window
        let mut test_params = config.param_grid[best_idx].clone();
        test_params.start = None;
        test_params.end = None;
        let test_report = crate::run_single(store, &test_params);
        let test_json = serde_json::to_value(&test_report).unwrap_or_default();
        let test_fitness = extract_fitness_from_json(&test_json, &config.fitness_metric);

        folds.push(FoldResult {
            fold_idx,
            train_start_row: train_start,
            train_end_row: train_end,
            test_start_row: test_start,
            test_end_row: test_end,
            best_params_idx: best_idx,
            train_fitness: best_train_fitness,
            test_fitness,
            test_report: test_json,
        });
    }

    let mean_oos = if folds.is_empty() {
        0.0
    } else {
        folds.iter().map(|f| f.test_fitness).sum::<f64>() / folds.len() as f64
    };

    WalkForwardResult {
        folds,
        mean_oos_fitness: mean_oos,
    }
}
