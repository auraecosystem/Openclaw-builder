//! Unit tests for all 7 functions in `algotrader_engine::analysis::metrics`.
//!
//! Each test uses hand-computed expected values. Float comparisons use a
//! tolerance of 1e-6 throughout.

use algotrader_engine::analysis::metrics::{
    cagr, equity_to_returns, estimate_annualization_from_dates, max_drawdown, profit_factor,
    sharpe, sortino,
};

const EPS: f64 = 1e-6;

// ---------------------------------------------------------------------------
// sharpe
// ---------------------------------------------------------------------------

#[test]
fn sharpe_known_returns() {
    // Returns: [0.01, -0.02, 0.03, -0.01, 0.02]
    // mean  = 0.006
    // sample var = sum((r - 0.006)^2) / (5 - 1)   (Bessel's correction)
    //           = 0.001720 / 4 = 0.000430
    // std   = sqrt(0.000430) = 0.020736441...
    // sharpe = 0.006 / 0.020736441 * 1.0 = 0.28934...
    let returns = vec![0.01, -0.02, 0.03, -0.01, 0.02];
    let result = sharpe(&returns, 1.0);
    let sample_var: f64 = 0.001720 / 4.0;
    let expected = 0.006 / sample_var.sqrt();
    assert!(
        (result - expected).abs() < EPS,
        "sharpe_known_returns: got {}, expected {}",
        result,
        expected,
    );
}

#[test]
fn sharpe_with_annualization() {
    // Same returns, annualized with sqrt(252), using sample std dev
    let returns = vec![0.01, -0.02, 0.03, -0.01, 0.02];
    let af = 252.0_f64.sqrt();
    let result = sharpe(&returns, af);
    let sample_var: f64 = 0.001720 / 4.0;
    let expected = (0.006 / sample_var.sqrt()) * af;
    assert!(
        (result - expected).abs() < EPS,
        "sharpe_with_annualization: got {}, expected {}",
        result,
        expected,
    );
}

#[test]
fn sharpe_zero_std_returns_zero() {
    // All identical returns: std = 0 -> sharpe = 0
    let returns = vec![0.01, 0.01, 0.01, 0.01];
    assert!((sharpe(&returns, 1.0)).abs() < EPS);
}

#[test]
fn sharpe_single_element_returns_zero() {
    // Fewer than 2 elements -> 0.0
    let returns = vec![0.05];
    assert!((sharpe(&returns, 1.0)).abs() < EPS);
}

#[test]
fn sharpe_empty_returns_zero() {
    assert!((sharpe(&[], 1.0)).abs() < EPS);
}

// ---------------------------------------------------------------------------
// sortino
// ---------------------------------------------------------------------------

#[test]
fn sortino_mixed_returns() {
    // Returns: [0.01, -0.02, 0.03, -0.01, 0.02]
    // mean = 0.006
    // downside_sum = (-0.02)^2 + (-0.01)^2 = 0.0004 + 0.0001 = 0.0005
    // downside_dev = sqrt(0.0005 / 5) = sqrt(0.0001) = 0.01
    // sortino = 0.006 / 0.01 * 1.0 = 0.6
    let returns = vec![0.01, -0.02, 0.03, -0.01, 0.02];
    let result = sortino(&returns, 1.0);
    assert!(
        (result - 0.6).abs() < EPS,
        "sortino_mixed: got {}, expected 0.6",
        result,
    );
}

#[test]
fn sortino_all_positive_returns_zero() {
    // No negative returns -> downside_dev = 0 -> sortino = 0
    let returns = vec![0.01, 0.02, 0.03, 0.01];
    assert!((sortino(&returns, 1.0)).abs() < EPS);
}

#[test]
fn sortino_all_negative() {
    // Returns: [-0.01, -0.02, -0.03]
    // mean = -0.02
    // downside_sum = 0.0001 + 0.0004 + 0.0009 = 0.0014
    // downside_dev = sqrt(0.0014 / 3) = sqrt(0.000466667) = 0.0215985...
    // sortino = -0.02 / 0.0215985 * 1.0 = -0.92604...
    let returns = vec![-0.01, -0.02, -0.03];
    let result = sortino(&returns, 1.0);
    let expected = -0.02 / (0.0014_f64 / 3.0).sqrt();
    assert!(
        (result - expected).abs() < EPS,
        "sortino_all_negative: got {}, expected {}",
        result,
        expected,
    );
}

#[test]
fn sortino_single_element_returns_zero() {
    assert!((sortino(&[0.05], 1.0)).abs() < EPS);
}

// ---------------------------------------------------------------------------
// cagr
// ---------------------------------------------------------------------------

#[test]
fn cagr_doubling_in_one_year() {
    // (200000 / 100000)^(1/1) - 1 = 1.0
    let result = cagr(100_000.0, 200_000.0, 1.0);
    assert!(
        (result - 1.0).abs() < EPS,
        "cagr_double_1yr: got {}, expected 1.0",
        result,
    );
}

#[test]
fn cagr_doubling_in_ten_years() {
    // (200000 / 100000)^(1/10) - 1 = 2^(0.1) - 1 ~= 0.071773...
    let result = cagr(100_000.0, 200_000.0, 10.0);
    let expected = 2.0_f64.powf(0.1) - 1.0;
    assert!(
        (result - expected).abs() < EPS,
        "cagr_double_10yr: got {}, expected {}",
        result,
        expected,
    );
}

#[test]
fn cagr_zero_years_returns_zero() {
    assert!((cagr(100_000.0, 200_000.0, 0.0)).abs() < EPS);
}

#[test]
fn cagr_zero_initial_returns_zero() {
    assert!((cagr(0.0, 200_000.0, 5.0)).abs() < EPS);
}

#[test]
fn cagr_zero_final_returns_zero() {
    assert!((cagr(100_000.0, 0.0, 5.0)).abs() < EPS);
}

#[test]
fn cagr_negative_years_returns_zero() {
    assert!((cagr(100_000.0, 200_000.0, -1.0)).abs() < EPS);
}

// ---------------------------------------------------------------------------
// max_drawdown
// ---------------------------------------------------------------------------

#[test]
fn max_drawdown_monotonic_up() {
    let curve = vec![100.0, 110.0, 120.0, 130.0, 140.0];
    assert!((max_drawdown(&curve)).abs() < EPS);
}

#[test]
fn max_drawdown_single_drop() {
    // Peak=100, trough=80 -> dd = 20/100 = 0.20
    let curve = vec![100.0, 80.0, 90.0, 95.0];
    assert!(
        (max_drawdown(&curve) - 0.20).abs() < EPS,
        "single drop dd: got {}",
        max_drawdown(&curve),
    );
}

#[test]
fn max_drawdown_multiple_peaks() {
    // First peak=100, drop to 90 (dd=10%), then new peak=120, drop to 84 (dd=30%)
    let curve = vec![100.0, 90.0, 120.0, 84.0, 100.0];
    assert!(
        (max_drawdown(&curve) - 0.30).abs() < EPS,
        "multi peak dd: got {}",
        max_drawdown(&curve),
    );
}

#[test]
fn max_drawdown_empty_returns_zero() {
    assert!((max_drawdown(&[])).abs() < EPS);
}

#[test]
fn max_drawdown_single_element() {
    // No drawdown possible with one point
    assert!((max_drawdown(&[100.0])).abs() < EPS);
}

#[test]
fn max_drawdown_total_loss() {
    // Peak=100, drops to 0 -> dd = 1.0
    let curve = vec![100.0, 50.0, 0.0];
    assert!(
        (max_drawdown(&curve) - 1.0).abs() < EPS,
        "total loss dd: got {}",
        max_drawdown(&curve),
    );
}

// ---------------------------------------------------------------------------
// profit_factor
// ---------------------------------------------------------------------------

#[test]
fn profit_factor_normal() {
    // 10000 profit / 5000 loss = 2.0
    assert!((profit_factor(10_000.0, 5_000.0) - 2.0).abs() < EPS);
}

#[test]
fn profit_factor_zero_loss_returns_max() {
    // All winners: loss = 0 -> f64::MAX
    assert_eq!(profit_factor(10_000.0, 0.0), f64::MAX);
}

#[test]
fn profit_factor_zero_profit_returns_zero() {
    assert!((profit_factor(0.0, 5_000.0)).abs() < EPS);
}

#[test]
fn profit_factor_both_zero() {
    // No trades: both zero -> 0.0
    assert!((profit_factor(0.0, 0.0)).abs() < EPS);
}

// ---------------------------------------------------------------------------
// equity_to_returns
// ---------------------------------------------------------------------------

#[test]
fn equity_to_returns_simple_sequence() {
    // [100, 110, 121] -> [0.10, 0.10]
    let curve = vec![100.0, 110.0, 121.0];
    let rets = equity_to_returns(&curve);
    assert_eq!(rets.len(), 2);
    assert!((rets[0] - 0.10).abs() < EPS, "ret[0]: got {}", rets[0]);
    assert!((rets[1] - 0.10).abs() < EPS, "ret[1]: got {}", rets[1]);
}

#[test]
fn equity_to_returns_with_loss() {
    // [100, 90, 99] -> [-0.10, 0.10]
    let curve = vec![100.0, 90.0, 99.0];
    let rets = equity_to_returns(&curve);
    assert_eq!(rets.len(), 2);
    assert!((rets[0] - (-0.10)).abs() < EPS, "ret[0]: got {}", rets[0]);
    assert!((rets[1] - 0.10).abs() < EPS, "ret[1]: got {}", rets[1]);
}

#[test]
fn equity_to_returns_single_point() {
    let rets = equity_to_returns(&[100.0]);
    assert!(rets.is_empty());
}

#[test]
fn equity_to_returns_empty() {
    let rets = equity_to_returns(&[]);
    assert!(rets.is_empty());
}

// ---------------------------------------------------------------------------
// estimate_annualization_from_dates
// ---------------------------------------------------------------------------

#[test]
fn estimate_annualization_one_year_daily() {
    // 365 calendar days, 252 observations (trading days)
    // years = 365 / 365.25 ~= 0.999315
    // obs_per_year = 252 / 0.999315 ~= 252.173
    // factor = sqrt(252.173) ~= 15.880
    let result = estimate_annualization_from_dates(0, 365, 252);
    let expected = (252.0_f64 / (365.0 / 365.25)).sqrt();
    assert!(
        (result - expected).abs() < 1e-3,
        "annualization_1yr: got {}, expected {}",
        result,
        expected,
    );
}

#[test]
fn estimate_annualization_half_year() {
    // 183 days, 126 observations
    let result = estimate_annualization_from_dates(0, 183, 126);
    let years = 183.0 / 365.25;
    let obs_per_year: f64 = 126.0 / years;
    let expected = obs_per_year.sqrt();
    assert!(
        (result - expected).abs() < 1e-3,
        "annualization_half: got {}, expected {}",
        result,
        expected,
    );
}

#[test]
fn estimate_annualization_zero_span_fallback() {
    // Same date -> fallback to sqrt(252)
    let result = estimate_annualization_from_dates(100, 100, 10);
    let expected = 252.0_f64.sqrt();
    assert!(
        (result - expected).abs() < EPS,
        "annualization_zero_span: got {}, expected {}",
        result,
        expected,
    );
}

#[test]
fn estimate_annualization_zero_observations_fallback() {
    let result = estimate_annualization_from_dates(0, 365, 0);
    let expected = 252.0_f64.sqrt();
    assert!(
        (result - expected).abs() < EPS,
        "annualization_zero_obs: got {}, expected {}",
        result,
        expected,
    );
}
