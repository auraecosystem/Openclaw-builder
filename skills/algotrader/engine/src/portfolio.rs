//! Multi-strategy portfolio runner.
//!
//! Loads all `*.json` configs from a directory, runs each strategy against
//! the same `DataStore` in parallel, then computes a pairwise correlation
//! matrix from the daily equity curves.
//!
//! Output is a `PortfolioReport` containing per-strategy `Report`s and the
//! N×N correlation matrix (indexed by strategy name).

use std::fs;
use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;

use engine_data::DataStore;
use engine_types::Params;

use crate::analysis::{
    correlation::{correlation_matrix, to_daily_equity},
    generate_report, Report,
};
use crate::run_core;

/// Output of `run_portfolio`: per-strategy reports + pairwise correlations.
#[derive(Serialize)]
pub struct PortfolioReport {
    /// Strategy name → individual backtest report.
    pub strategies: Vec<StrategyEntry>,
    /// N×N Pearson correlation matrix of daily log-returns.
    /// Indexed by the same order as `strategies`.
    pub correlation: Vec<Vec<f64>>,
    /// Combined metrics across all strategies (simple averages of key stats).
    pub combined: CombinedMetrics,
}

#[derive(Serialize)]
pub struct StrategyEntry {
    pub name: String,
    pub report: Report,
}

#[derive(Serialize)]
pub struct CombinedMetrics {
    pub total_trades: usize,
    pub avg_sharpe: f64,
    pub avg_profit_factor: f64,
    pub avg_max_drawdown: f64,
    pub avg_cagr: f64,
}

/// Run all `*.json` strategy configs in `config_dir` against `store` and
/// return combined portfolio metrics + pairwise correlation matrix.
///
/// Each JSON file is deserialized as `Params`. The strategy name is the
/// filename stem (e.g. `breakout_quick.json` → `"breakout_quick"`).
pub fn run_portfolio(store: &DataStore, config_dir: &Path) -> Result<PortfolioReport> {
    let entries = collect_configs(config_dir)?;
    if entries.is_empty() {
        anyhow::bail!("No *.json configs found in {}", config_dir.display());
    }

    eprintln!("  Portfolio: {} strategies", entries.len());

    let n_rows = store.axes.n_rows;

    // Run all strategies in parallel; collect (name, params, report, equity_curve)
    let results: Vec<(String, Report)> = entries
        .par_iter()
        .map(|(name, params)| {
            let mut p = params.clone();
            // Always emit equity curve internally so we can correlate
            p.analysis.emit_equity_curve = true;
            let (trades, _, _) = run_core(store, &p);
            let report = generate_report(&trades, &store.axes, &p, 1);
            (name.clone(), report)
        })
        .collect();

    // Build daily equity curves for correlation
    let daily_curves: Vec<Vec<f64>> = results
        .iter()
        .map(|(_, report)| {
            if let Some(ref ec) = report.equity_curve {
                to_daily_equity(ec, n_rows, report.init_cash)
            } else {
                vec![report.init_cash; n_rows]
            }
        })
        .collect();

    let corr = correlation_matrix(&daily_curves);

    // Combined metrics (simple averages over strategies with at least 1 trade)
    let active: Vec<&Report> = results
        .iter()
        .map(|(_, r)| r)
        .filter(|r| r.total_trades > 0)
        .collect();

    // Filter NaN/infinite values when averaging so one degenerate strategy
    // doesn't poison the combined metrics.
    let avg_finite = |f: fn(&Report) -> f64| -> f64 {
        let vals: Vec<f64> = active.iter().map(|r| f(r)).filter(|v| v.is_finite()).collect();
        if vals.is_empty() { 0.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 }
    };
    let combined = CombinedMetrics {
        total_trades: active.iter().map(|r| r.total_trades).sum(),
        avg_sharpe: avg_finite(|r| r.sharpe),
        avg_profit_factor: avg_finite(|r| r.profit_factor),
        avg_max_drawdown: avg_finite(|r| r.max_drawdown),
        avg_cagr: avg_finite(|r| r.cagr),
    };

    // Strip internal equity curve from final output (not needed in portfolio JSON)
    let strategies: Vec<StrategyEntry> = results
        .into_iter()
        .map(|(name, mut report)| {
            report.equity_curve = None;
            StrategyEntry { name, report }
        })
        .collect();

    Ok(PortfolioReport {
        strategies,
        correlation: corr,
        combined,
    })
}

/// Collect all `*.json` files in `dir` as `(name_stem, Params)` pairs.
fn collect_configs(dir: &Path) -> Result<Vec<(String, Params)>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let content = fs::read_to_string(&path)?;
            let params: Params = serde_json::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path.display(), e))?;
            entries.push((name, params));
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
}
