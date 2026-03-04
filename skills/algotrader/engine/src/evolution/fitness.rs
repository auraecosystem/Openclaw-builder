//! Fitness evaluation: wraps run_single() and extracts a scalar metric.
//!
//! Direct port of the Python `evolve.py` fitness function. Applies a
//! minimum-trade-count gate and a trade-confidence ramp so the optimizer
//! does not chase overfitted parameter sets that fire rarely.

use engine_data::DataStore;
use engine_types::{FitnessConfig, Params};

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
    fitness_cfg: &FitnessConfig,
) -> (serde_json::Value, f64) {
    let report = crate::run_single(store, params);
    let report_json = serde_json::to_value(&report).unwrap_or_default();
    let score = extract_fitness(&report_json, metric, fitness_cfg);
    (report_json, score)
}

/// Run a backtest with pipeline param overrides for dynamic JSON strategies.
///
/// Re-creates the DynamicSetup with overridden params each call. The overhead
/// is negligible vs. a full backtest (~1ms parse vs ~3s simulation).
pub fn evaluate_dynamic(
    store: &DataStore,
    base: &Params,
    overrides: &std::collections::HashMap<String, serde_json::Value>,
    metric: &FitnessMetric,
    fitness_cfg: &FitnessConfig,
) -> (serde_json::Value, f64) {
    let report = crate::run_single_dynamic(store, base, overrides);
    let report_json = serde_json::to_value(&report).unwrap_or_default();
    let score = extract_fitness(&report_json, metric, fitness_cfg);
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
pub fn extract_fitness(
    report: &serde_json::Value,
    metric: &FitnessMetric,
    cfg: &FitnessConfig,
) -> f64 {
    let trades = report["total_trades"].as_u64().unwrap_or(0) as usize;
    if (trades as u64) < cfg.min_trades_gate {
        return cfg.penalty_score;
    }

    // Linear ramp: confidence_base at min_trades_gate, 1.0 at gate + range.
    let trade_confidence = (cfg.confidence_base
        + cfg.confidence_slope * (trades as f64 - cfg.min_trades_gate as f64) / cfg.confidence_range)
        .min(1.0);

    // Sharpe overfit guard: scale the ceiling with trade count because
    // many trades in a bull market can legitimately produce higher Sharpe.
    // Floor at sharpe_cap_base (few trades = easy to game), grows with log10(trades).
    let sharpe_val = report["sharpe"].as_f64().unwrap_or(0.0);
    let sharpe_cap = cfg.sharpe_cap_base + (trades as f64).log10();
    if sharpe_val > sharpe_cap {
        return cfg.penalty_score;
    }

    let score = match metric {
        FitnessMetric::Sharpe => report["sharpe"].as_f64().unwrap_or(cfg.penalty_score),

        FitnessMetric::Sortino => report["sortino"].as_f64().unwrap_or(cfg.penalty_score),

        FitnessMetric::Return => report["total_return"].as_f64().unwrap_or(cfg.penalty_score),

        FitnessMetric::ProfitFactor => report["profit_factor"].as_f64().unwrap_or(0.0),

        FitnessMetric::Cagr => report["cagr"].as_f64().unwrap_or(cfg.penalty_score),

        FitnessMetric::Growth => {
            let init = report["init_cash"].as_f64().unwrap_or(1.0);
            let final_eq = report["final_equity"].as_f64().unwrap_or(init);
            if init > 0.0 {
                final_eq / init
            } else {
                cfg.penalty_score
            }
        }

        FitnessMetric::Composite => {
            let sharpe = report["sharpe"].as_f64().unwrap_or(0.0);
            let ret = report["total_return"].as_f64().unwrap_or(0.0);
            let wr = report["win_rate"].as_f64().unwrap_or(0.0);
            let dd = report["max_drawdown"].as_f64().unwrap_or(1.0);

            // Penalize strategies where drawdown exceeds a threshold.
            let dd_penalty = (1.0 - dd * cfg.drawdown_penalty_mult).max(0.0);

            (sharpe * cfg.composite_w_sharpe
                + ret * cfg.composite_return_scale * cfg.composite_w_return
                + wr * cfg.composite_w_winrate)
                * dd_penalty
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

    fn default_cfg() -> FitnessConfig {
        FitnessConfig::default()
    }

    #[test]
    fn low_trade_count_penalized() {
        let report = serde_json::json!({
            "total_trades": 5,
            "sharpe": 2.0,
            "total_return": 0.5,
            "win_rate": 0.6,
            "max_drawdown": 0.1,
        });
        assert_eq!(extract_fitness(&report, &FitnessMetric::Sharpe, &default_cfg()), -999.0);
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
        let score = extract_fitness(&report, &FitnessMetric::Composite, &default_cfg());
        assert!((score - 1.092).abs() < 0.01, "got {score}");
    }

    #[test]
    fn trade_confidence_ramp() {
        let cfg = default_cfg();
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

        let f10 = extract_fitness(&r10, &FitnessMetric::Sharpe, &cfg);
        let f50 = extract_fitness(&r50, &FitnessMetric::Sharpe, &cfg);

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
        let score = extract_fitness(&report, &FitnessMetric::Growth, &default_cfg());
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
        let score = extract_fitness(&report, &FitnessMetric::Growth, &default_cfg());
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
        assert_eq!(extract_fitness(&report, &FitnessMetric::Composite, &default_cfg()), -999.0);
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
        assert!(extract_fitness(&report, &FitnessMetric::Composite, &default_cfg()) > 0.0);
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
        assert!(extract_fitness(&report, &FitnessMetric::Sharpe, &default_cfg()) > 0.0);
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
        let score = extract_fitness(&report, &FitnessMetric::Composite, &default_cfg());
        assert!(
            score.abs() < 1e-9,
            "60% drawdown should zero out composite, got {score}"
        );
    }
}
