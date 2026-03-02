use serde::Serialize;

use super::metrics;
use engine_data::Axes;
use engine_types::{Params, Trade};

/// Equity curve sampled at each trade exit, aligned by row index.
///
/// `values[i]` is the running equity after the i-th exit (length = n_trades + 1;
/// index 0 is init_cash before any trade). `exit_rows[i]` is the daily row of
/// the (i+1)-th exit so callers can align to a shared date grid.
#[derive(Serialize, Clone)]
pub struct EquityCurve {
    pub values: Vec<f64>,
    pub exit_rows: Vec<usize>,
}

#[derive(Serialize)]
pub struct Report {
    pub total_trades: usize,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub avg_hold_days: f64,
    pub total_return: f64,
    pub cagr: f64,
    pub max_drawdown: f64,
    pub sharpe: f64,
    pub sortino: f64,
    pub init_cash: f64,
    pub final_equity: f64,
    /// Probabilistic Sharpe Ratio: P(true Sharpe > 0) given observed skew/kurtosis.
    /// Above 0.95 is conventionally significant.
    pub psr: f64,
    /// Deflated Sharpe Ratio: PSR adjusted for multiple testing (N trials from params).
    /// Requires `n_trials` > 1 in params; otherwise equals PSR.
    pub dsr: f64,
    pub per_setup: serde_json::Value,
    pub params: Params,
    /// Equity curve (omitted from JSON unless `params.analysis.emit_equity_curve`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equity_curve: Option<EquityCurve>,
}

/// `n_trials`: number of independent parameter sets tested (for DSR multiple-testing correction).
/// Pass 1 for a single run (DSR == PSR).
pub fn generate_report(trades: &[Trade], axes: &Axes, params: &Params, n_trials: usize) -> Report {
    let n = trades.len();
    if n == 0 {
        return Report {
            total_trades: 0,
            win_rate: 0.0,
            profit_factor: 0.0,
            avg_win: 0.0,
            avg_loss: 0.0,
            avg_hold_days: 0.0,
            total_return: 0.0,
            cagr: 0.0,
            max_drawdown: 0.0,
            sharpe: 0.0,
            sortino: 0.0,
            init_cash: params.init_cash,
            final_equity: params.init_cash,
            psr: 0.0,
            dsr: 0.0,
            per_setup: serde_json::json!({}),
            params: params.clone(),
            equity_curve: None,
        };
    }

    let wins: Vec<&Trade> = trades.iter().filter(|t| t.pnl > 0.0).collect();
    let losses: Vec<&Trade> = trades.iter().filter(|t| t.pnl <= 0.0).collect();

    let win_rate = wins.len() as f64 / n as f64;
    let gross_profit: f64 = wins.iter().map(|t| t.pnl).sum();
    let gross_loss: f64 = losses.iter().map(|t| t.pnl.abs()).sum();
    let pf = metrics::profit_factor(gross_profit, gross_loss);

    let avg_win = if !wins.is_empty() {
        gross_profit / wins.len() as f64
    } else {
        0.0
    };
    let avg_loss = if !losses.is_empty() {
        gross_loss / losses.len() as f64
    } else {
        0.0
    };

    let total_pnl: f64 = trades.iter().map(|t| t.pnl).sum();
    let final_equity = params.init_cash + total_pnl;
    let total_return = total_pnl / params.init_cash;

    // Hold time in calendar days (works correctly for both daily and 5m bars)
    let total_hold_days: f64 = trades
        .iter()
        .map(|t| {
            let d0 = axes.dates.get(t.entry_row).copied().unwrap_or(0);
            let d1 = axes.dates.get(t.exit_row).copied().unwrap_or(0);
            // If same day (e.g. intraday on 5m bars), count fractional days from row span
            let cal_days = (d1 - d0) as f64;
            if cal_days > 0.0 {
                cal_days
            } else {
                // Intraday: estimate fraction of day from row distance
                let rows = t.exit_row.saturating_sub(t.entry_row) as f64;
                rows / params.analysis.intraday_bars_per_day
            }
        })
        .sum();
    let avg_hold_days = total_hold_days / n as f64;

    // Build equity curve from trades sorted by exit_row
    let mut sorted_trades: Vec<&Trade> = trades.iter().collect();
    sorted_trades.sort_by_key(|t| t.exit_row);

    let mut equity_values: Vec<f64> = Vec::with_capacity(sorted_trades.len() + 1);
    let mut exit_rows: Vec<usize> = Vec::with_capacity(sorted_trades.len());
    equity_values.push(params.init_cash);
    let mut running_eq = params.init_cash;
    for t in &sorted_trades {
        running_eq += t.pnl;
        equity_values.push(running_eq);
        exit_rows.push(t.exit_row);
    }

    let max_dd = metrics::max_drawdown(&equity_values);

    // CAGR: use calendar days between first entry and last exit
    let cagr_val = if sorted_trades.len() >= 2 {
        let first_row = sorted_trades.first().map(|t| t.entry_row).unwrap_or(0);
        let last_row = sorted_trades.last().map(|t| t.exit_row).unwrap_or(0);
        let first_date = axes.dates.get(first_row).copied().unwrap_or(0);
        let last_date = axes.dates.get(last_row).copied().unwrap_or(0);
        let years = (last_date - first_date) as f64 / 365.25;
        metrics::cagr(params.init_cash, final_equity, years)
    } else {
        0.0
    };

    // Sharpe/Sortino/PSR/DSR from trade-level returns with calendar-based annualization
    // Minimum 3 trades for meaningful stats (skewness needs n>=3)
    let (sharpe_val, sortino_val, psr_val, dsr_val) = if equity_values.len() > 3 {
        let returns = metrics::equity_to_returns(&equity_values);
        let first_row = sorted_trades.first().map(|t| t.entry_row).unwrap_or(0);
        let last_row = sorted_trades.last().map(|t| t.exit_row).unwrap_or(0);
        let first_date = axes.dates.get(first_row).copied().unwrap_or(0);
        let last_date = axes.dates.get(last_row).copied().unwrap_or(0);
        let ann = metrics::estimate_annualization_from_dates(first_date, last_date, returns.len());
        let sr = metrics::sharpe(&returns, ann);
        let so = metrics::sortino(&returns, ann);
        // PSR/DSR use the non-annualized (per-trade) Sharpe, per Bailey & de Prado (2014).
        // The SE formula assumes the raw sample SR = mean/std without annualization.
        let sr_raw = metrics::sharpe(&returns, 1.0);
        let skew = metrics::skewness(&returns);
        let kurt = metrics::excess_kurtosis(&returns);
        let psr = metrics::probabilistic_sharpe(sr_raw, 0.0, returns.len(), skew, kurt);
        let dsr = metrics::deflated_sharpe(sr_raw, returns.len(), skew, kurt, n_trials.max(1));
        (sr, so, psr, dsr)
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };

    // Per-setup breakdown
    let mut setup_map = serde_json::Map::new();
    let mut setup_names: Vec<&str> = trades.iter().map(|t| t.setup).collect();
    setup_names.sort();
    setup_names.dedup();

    for setup_name in setup_names {
        let subset: Vec<&Trade> = trades.iter().filter(|t| t.setup == setup_name).collect();
        let sub_wins = subset.iter().filter(|t| t.pnl > 0.0).count();
        let sub_pnl: f64 = subset.iter().map(|t| t.pnl).sum();
        let sub_hold: f64 = subset
            .iter()
            .map(|t| {
                let d0 = axes.dates.get(t.entry_row).copied().unwrap_or(0);
                let d1 = axes.dates.get(t.exit_row).copied().unwrap_or(0);
                let cal_days = (d1 - d0) as f64;
                if cal_days > 0.0 {
                    cal_days
                } else {
                    t.exit_row.saturating_sub(t.entry_row) as f64
                        / params.analysis.intraday_bars_per_day
                }
            })
            .sum();
        setup_map.insert(
            setup_name.to_string(),
            serde_json::json!({
                "trades": subset.len(),
                "win_rate": round3(sub_wins as f64 / subset.len() as f64),
                "total_pnl": round2(sub_pnl),
                "avg_hold_days": round1(sub_hold / subset.len() as f64),
            }),
        );
    }

    let equity_curve = if params.analysis.emit_equity_curve {
        Some(EquityCurve { values: equity_values, exit_rows })
    } else {
        None
    };

    Report {
        total_trades: n,
        win_rate: round3(win_rate),
        profit_factor: round2(pf),
        avg_win: round2(avg_win),
        avg_loss: round2(avg_loss),
        avg_hold_days: round1(avg_hold_days),
        total_return: round4(total_return),
        cagr: round4(cagr_val),
        max_drawdown: round4(max_dd),
        sharpe: round2(sharpe_val),
        sortino: round2(sortino_val),
        init_cash: params.init_cash,
        final_equity: round2(final_equity),
        psr: round4(psr_val),
        dsr: round4(dsr_val),
        per_setup: serde_json::Value::Object(setup_map),
        params: params.clone(),
        equity_curve,
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}
