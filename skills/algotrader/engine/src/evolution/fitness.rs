//! Fitness evaluation: wraps run_single() and extracts a scalar metric.
//!
//! Direct port of the Python `evolve.py` fitness function. Applies a
//! minimum-trade-count gate and a trade-confidence ramp so the optimizer
//! does not chase overfitted parameter sets that fire rarely.

use crate::data::DataStore;
use crate::types::Params;

// ---------------------------------------------------------------------------
// Fitness metric selection
// ---------------------------------------------------------------------------

/// Which scalar metric the optimizer maximizes.
#[derive(Clone, Debug)]
pub enum FitnessMetric {
    Sharpe,
    Sortino,
    Return,
    ProfitFactor,
    Cagr,
    Growth,
    Composite,
}

impl FitnessMetric {
    /// Parse a CLI string into a metric variant.
    ///
    /// Unrecognized strings fall back to `Composite` -- the safest default
    /// because it balances risk-adjusted return with drawdown penalty.
    pub fn parse(s: &str) -> Self {
        match s {
            "sharpe" => Self::Sharpe,
            "sortino" => Self::Sortino,
            "return" => Self::Return,
            "profit_factor" | "pf" => Self::ProfitFactor,
            "cagr" => Self::Cagr,
            "growth" => Self::Growth,
            "composite" => Self::Composite,
            _ => Self::Composite,
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluation entry point
// ---------------------------------------------------------------------------

/// Run a backtest with the given params and return (report_json, fitness_score).
///
/// The report is serialized to `serde_json::Value` so callers can inspect
/// any field without coupling to the `Report` struct directly.
pub fn evaluate(
    store: &DataStore,
    params: &Params,
    metric: &FitnessMetric,
) -> (serde_json::Value, f64) {
    let report = crate::run_single(store, params);
    let report_json = serde_json::to_value(&report).unwrap_or_default();
    let score = extract_fitness(&report_json, metric);
    (report_json, score)
}

// ---------------------------------------------------------------------------
// Fitness extraction (ported from evolve.py)
// ---------------------------------------------------------------------------

/// Extract a scalar fitness from a serialized report.
///
/// Strategies with fewer than 10 trades get -999.0 (effectively dead).
/// Between 10 and 50 trades, a linear confidence ramp scales the score
/// from 0.3x to 1.0x so the optimizer cannot game the metric by firing
/// on a handful of cherry-picked setups.
pub fn extract_fitness(report: &serde_json::Value, metric: &FitnessMetric) -> f64 {
    let trades = report["total_trades"].as_u64().unwrap_or(0) as usize;
    if trades < 10 {
        return -999.0;
    }

    // Linear ramp: 0.3 at 10 trades, 1.0 at 50+ trades.
    let trade_confidence = (0.3 + 0.7 * (trades as f64 - 10.0) / 40.0).min(1.0);

    // Sharpe overfit guard: scale the ceiling with trade count because
    // many trades in a bull market can legitimately produce higher Sharpe.
    // Floor at 3.5 (few trades = easy to game), grows with log10(trades).
    let sharpe_val = report["sharpe"].as_f64().unwrap_or(0.0);
    let sharpe_cap = 3.5 + (trades as f64).log10();
    if sharpe_val > sharpe_cap {
        return -999.0;
    }

    let score = match metric {
        FitnessMetric::Sharpe => report["sharpe"].as_f64().unwrap_or(-999.0),

        FitnessMetric::Sortino => report["sortino"].as_f64().unwrap_or(-999.0),

        FitnessMetric::Return => report["total_return"].as_f64().unwrap_or(-999.0),

        FitnessMetric::ProfitFactor => report["profit_factor"].as_f64().unwrap_or(0.0),

        FitnessMetric::Cagr => report["cagr"].as_f64().unwrap_or(-999.0),

        FitnessMetric::Growth => {
            let init = report["init_cash"].as_f64().unwrap_or(1.0);
            let final_eq = report["final_equity"].as_f64().unwrap_or(init);
            if init > 0.0 {
                final_eq / init
            } else {
                -999.0
            }
        }

        FitnessMetric::Composite => {
            let sharpe = report["sharpe"].as_f64().unwrap_or(0.0);
            let ret = report["total_return"].as_f64().unwrap_or(0.0);
            let wr = report["win_rate"].as_f64().unwrap_or(0.0);
            let dd = report["max_drawdown"].as_f64().unwrap_or(1.0);

            // Penalize strategies where drawdown exceeds 50% of equity.
            let dd_penalty = (1.0 - dd * 2.0).max(0.0);

            (sharpe * 0.4 + ret * 10.0 * 0.3 + wr * 0.3) * dd_penalty
        }
    };

    score * trade_confidence
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_trade_count_penalized() {
        let report = serde_json::json!({
            "total_trades": 5,
            "sharpe": 2.0,
            "total_return": 0.5,
            "win_rate": 0.6,
            "max_drawdown": 0.1,
        });
        assert_eq!(extract_fitness(&report, &FitnessMetric::Sharpe), -999.0);
    }

    #[test]
    fn composite_matches_python() {
        // Verify composite formula matches evolve.py:
        // sharpe=1.5, ret=0.20, wr=0.55, dd=0.10 with 60 trades
        // dd_penalty = max(0, 1 - 0.10*2) = 0.80
        // raw = 1.5*0.4 + 0.20*10*0.3 + 0.55*0.3 = 0.6 + 0.6 + 0.165 = 1.365
        // score = 1.365 * 0.80 = 1.092
        // trade_confidence = min(1.0, 0.3 + 0.7*(60-10)/40) = 1.0
        // final = 1.092 * 1.0 = 1.092
        let report = serde_json::json!({
            "total_trades": 60,
            "sharpe": 1.5,
            "total_return": 0.20,
            "win_rate": 0.55,
            "max_drawdown": 0.10,
            "init_cash": 100000.0,
            "final_equity": 120000.0,
        });
        let score = extract_fitness(&report, &FitnessMetric::Composite);
        assert!((score - 1.092).abs() < 0.01, "got {score}");
    }

    #[test]
    fn trade_confidence_ramp() {
        let base = serde_json::json!({
            "sharpe": 2.0,
            "total_return": 0.3,
            "win_rate": 0.5,
            "max_drawdown": 0.1,
        });

        // 10 trades -> confidence = 0.3
        let r10 = {
            let mut r = base.clone();
            r["total_trades"] = serde_json::json!(10);
            r
        };
        // 50 trades -> confidence = 1.0
        let r50 = {
            let mut r = base.clone();
            r["total_trades"] = serde_json::json!(50);
            r
        };

        let f10 = extract_fitness(&r10, &FitnessMetric::Sharpe);
        let f50 = extract_fitness(&r50, &FitnessMetric::Sharpe);

        assert!(f50 > f10, "more trades should have higher confidence");
        assert!(
            (f10 - 2.0 * 0.3).abs() < 0.01,
            "10 trades: expected {}, got {f10}",
            2.0 * 0.3
        );
        assert!(
            (f50 - 2.0).abs() < 0.01,
            "50 trades: expected 2.0, got {f50}"
        );
    }

    #[test]
    fn growth_metric() {
        let report = serde_json::json!({
            "total_trades": 100,
            "init_cash": 100000.0,
            "final_equity": 150000.0,
        });
        let score = extract_fitness(&report, &FitnessMetric::Growth);
        assert!((score - 1.5).abs() < 0.01);
    }

    #[test]
    fn metric_from_str() {
        assert!(matches!(
            FitnessMetric::parse("sharpe"),
            FitnessMetric::Sharpe
        ));
        assert!(matches!(
            FitnessMetric::parse("composite"),
            FitnessMetric::Composite
        ));
        assert!(matches!(
            FitnessMetric::parse("pf"),
            FitnessMetric::ProfitFactor
        ));
        assert!(matches!(
            FitnessMetric::parse("unknown"),
            FitnessMetric::Composite
        ));
    }

    #[test]
    fn zero_init_cash_returns_penalty() {
        let report = serde_json::json!({
            "total_trades": 50,
            "init_cash": 0.0,
            "final_equity": 150000.0,
        });
        let score = extract_fitness(&report, &FitnessMetric::Growth);
        assert_eq!(score, -999.0);
    }

    #[test]
    fn high_sharpe_low_trades_killed() {
        // 37 trades: cap = 3.5 + log10(37) ≈ 5.07. Sharpe 35 >> cap → killed.
        let report = serde_json::json!({
            "total_trades": 37,
            "sharpe": 35.0,
            "total_return": 0.24,
            "win_rate": 0.75,
            "max_drawdown": 0.03,
        });
        assert_eq!(extract_fitness(&report, &FitnessMetric::Composite), -999.0);
    }

    #[test]
    fn moderate_sharpe_high_trades_survives() {
        // 1000 trades: cap = 3.5 + 3.0 = 6.5. Sharpe 5.0 < cap → survives.
        let report = serde_json::json!({
            "total_trades": 1000,
            "sharpe": 5.0,
            "total_return": 0.5,
            "win_rate": 0.55,
            "max_drawdown": 0.15,
        });
        assert!(extract_fitness(&report, &FitnessMetric::Composite) > 0.0);
    }

    #[test]
    fn borderline_sharpe_survives() {
        // 100 trades: cap = 3.5 + 2.0 = 5.5. Sharpe 3.5 < cap → survives.
        let report = serde_json::json!({
            "total_trades": 100,
            "sharpe": 3.5,
            "total_return": 0.5,
            "win_rate": 0.6,
            "max_drawdown": 0.1,
        });
        assert!(extract_fitness(&report, &FitnessMetric::Sharpe) > 0.0);
    }

    #[test]
    fn high_drawdown_kills_composite() {
        // drawdown = 0.60 -> dd_penalty = max(0, 1 - 1.2) = 0.0
        let report = serde_json::json!({
            "total_trades": 100,
            "sharpe": 3.0,
            "total_return": 1.0,
            "win_rate": 0.70,
            "max_drawdown": 0.60,
        });
        let score = extract_fitness(&report, &FitnessMetric::Composite);
        assert!(
            score.abs() < 1e-9,
            "60% drawdown should zero out composite, got {score}"
        );
    }
}
