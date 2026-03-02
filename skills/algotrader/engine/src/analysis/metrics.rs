//! Pure financial math — no business logic, no data structures.
//!
//! All functions are stateless and operate on slices. Used by report.rs
//! to compute performance metrics from equity curves and trade data.

/// Sharpe ratio from periodic returns.
///
/// `annualization_factor` = sqrt(observations_per_year).
/// For daily returns: sqrt(252). For trade-level returns: sqrt(trades_per_year).
pub fn sharpe(returns: &[f64], annualization_factor: f64) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }
    let n = returns.len() as f64;
    let mean = returns.iter().sum::<f64>() / n;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let std = variance.sqrt();
    if std > 0.0 {
        (mean / std) * annualization_factor
    } else {
        0.0
    }
}

/// Sortino ratio — uses target downside deviation with ALL observations in denominator.
///
/// Downside deviation = sqrt(sum(min(r, 0)^2) / N) where N = total count,
/// not just the count of negative returns. This is the correct formulation
/// per Sortino & Price (1994) and the CFA Institute.
pub fn sortino(returns: &[f64], annualization_factor: f64) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }
    let n = returns.len() as f64;
    let mean = returns.iter().sum::<f64>() / n;

    // Sum of squared negative returns, divided by total N (not just negative count)
    let downside_sum: f64 = returns
        .iter()
        .filter(|&&r| r < 0.0)
        .map(|r| r.powi(2))
        .sum();
    let downside_dev = (downside_sum / n).sqrt();

    if downside_dev > 0.0 {
        (mean / downside_dev) * annualization_factor
    } else {
        0.0
    }
}

/// Compound annual growth rate.
///
/// CAGR = (final / initial)^(1/years) - 1
pub fn cagr(initial: f64, final_eq: f64, years: f64) -> f64 {
    if years > 0.0 && initial > 0.0 && final_eq > 0.0 {
        (final_eq / initial).powf(1.0 / years) - 1.0
    } else {
        0.0
    }
}

/// Maximum drawdown from an equity curve (peak-to-trough / peak).
///
/// Returns a value in [0, 1] where 0 = no drawdown, 1 = total loss.
pub fn max_drawdown(equity_curve: &[f64]) -> f64 {
    if equity_curve.is_empty() {
        return 0.0;
    }
    let mut peak = equity_curve[0];
    let mut max_dd = 0.0_f64;
    for &eq in equity_curve {
        if eq > peak {
            peak = eq;
        }
        if peak > 0.0 {
            let dd = (peak - eq) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }
    max_dd
}

/// Profit factor = gross_profit / gross_loss.
///
/// Returns f64::MAX when gross_loss == 0 (all trades are winners).
/// Returns 0.0 when gross_profit == 0 (all trades are losers).
pub fn profit_factor(gross_profit: f64, gross_loss: f64) -> f64 {
    if gross_loss > 0.0 {
        gross_profit / gross_loss
    } else if gross_profit > 0.0 {
        f64::MAX
    } else {
        0.0
    }
}

/// Sample skewness (Fisher's definition).
pub fn skewness(returns: &[f64]) -> f64 {
    let n = returns.len() as f64;
    if n < 3.0 {
        return 0.0;
    }
    let mean = returns.iter().sum::<f64>() / n;
    let m2: f64 = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
    let m3: f64 = returns.iter().map(|r| (r - mean).powi(3)).sum::<f64>() / n;
    let std = m2.sqrt();
    if std > 0.0 {
        m3 / std.powi(3)
    } else {
        0.0
    }
}

/// Sample excess kurtosis (Fisher's definition, normal = 0).
pub fn excess_kurtosis(returns: &[f64]) -> f64 {
    let n = returns.len() as f64;
    if n < 4.0 {
        return 0.0;
    }
    let mean = returns.iter().sum::<f64>() / n;
    let m2: f64 = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
    let m4: f64 = returns.iter().map(|r| (r - mean).powi(4)).sum::<f64>() / n;
    if m2 > 0.0 {
        m4 / m2.powi(2) - 3.0
    } else {
        0.0
    }
}

/// Standard error of the Sharpe ratio accounting for skewness and kurtosis.
///
/// From Lo (2002) and Bailey & de Prado (2014):
/// SE(SR) = sqrt((1 - skew*SR + (kurt_excess+2)/4 * SR^2) / (n-1))
pub fn sharpe_std_err(sr: f64, n: usize, skew: f64, excess_kurt: f64) -> f64 {
    if n < 2 {
        return f64::MAX;
    }
    let n_f = n as f64;
    let numerator = 1.0 - skew * sr + (excess_kurt + 2.0) / 4.0 * sr.powi(2);
    (numerator.max(0.0) / (n_f - 1.0)).sqrt()
}

/// Probabilistic Sharpe Ratio: P(true SR > sr_benchmark).
///
/// PSR = Phi((SR - SR_benchmark) / SE(SR))
/// where Phi is the standard normal CDF.
/// Returns a value in [0, 1]. Above 0.95 is conventionally "significant."
pub fn probabilistic_sharpe(sr: f64, sr_benchmark: f64, n: usize, skew: f64, excess_kurt: f64) -> f64 {
    let se = sharpe_std_err(sr, n, skew, excess_kurt);
    if se <= 0.0 || se == f64::MAX {
        return if sr > sr_benchmark { 1.0 } else { 0.0 };
    }
    let z = (sr - sr_benchmark) / se;
    norm_cdf(z)
}

/// Deflated Sharpe Ratio: adjusts for N independent trials (multiple testing).
///
/// Bailey & de Prado (2014), Eq. 6:
///   SR_0 = sqrt(V[SR_hat]) * Z*
/// where Z* = (1 - γ/(2·ln(N)))·sqrt(2·ln(N)) - sqrt(π/(2·ln(N)))/2
/// and V[SR_hat] ≈ 1/(T-1) under the null (true SR = 0).
///
/// DSR = PSR with sr_benchmark = SR_0.
pub fn deflated_sharpe(sr: f64, n_returns: usize, skew: f64, excess_kurt: f64, n_trials: usize) -> f64 {
    if n_trials <= 1 {
        return probabilistic_sharpe(sr, 0.0, n_returns, skew, excess_kurt);
    }
    let n = n_trials as f64;
    let euler_mascheroni = 0.5772156649;
    // Z* = expected max of N standard normals (with finite-N correction)
    let z_star = (2.0 * n.ln()).sqrt()
        * (1.0 - euler_mascheroni / (2.0 * n.ln()))
        - 0.5 * (std::f64::consts::PI / (2.0 * n.ln())).sqrt();
    // Under the null (true SR = 0), V[SR_hat] ≈ 1/(T-1)
    let se_null = if n_returns > 1 {
        (1.0 / (n_returns as f64 - 1.0)).sqrt()
    } else {
        1.0
    };
    let sr_0 = se_null * z_star;
    probabilistic_sharpe(sr, sr_0.max(0.0), n_returns, skew, excess_kurt)
}

/// Standard normal CDF approximation (Abramowitz & Stegun 26.2.17, |error| < 7.5e-8).
fn norm_cdf(x: f64) -> f64 {
    if x < -8.0 {
        return 0.0;
    }
    if x > 8.0 {
        return 1.0;
    }
    let a1 = 0.319381530;
    let a2 = -0.356563782;
    let a3 = 1.781477937;
    let a4 = -1.821255978;
    let a5 = 1.330274429;
    let abs_x = x.abs();
    let t = 1.0 / (1.0 + 0.2316419 * abs_x);
    let pdf = (-0.5 * abs_x * abs_x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let cdf = 1.0 - pdf * t * (a1 + t * (a2 + t * (a3 + t * (a4 + t * a5))));
    if x >= 0.0 { cdf } else { 1.0 - cdf }
}

/// Compute periodic returns from an equity curve.
///
/// Returns (r[i] - r[i-1]) / r[i-1] for each consecutive pair.
pub fn equity_to_returns(equity_curve: &[f64]) -> Vec<f64> {
    equity_curve
        .windows(2)
        .map(|w| (w[1] - w[0]) / w[0])
        .collect()
}

/// Estimate annualization factor from calendar dates (timeframe-agnostic).
///
/// Uses actual calendar time span and observation count to compute
/// observations-per-year, then returns sqrt(obs_per_year) for Sharpe/Sortino.
/// Works correctly for daily, 5m, or any other bar frequency.
pub fn estimate_annualization_from_dates(
    first_date: i32, // days-since-epoch
    last_date: i32,
    n_observations: usize,
) -> f64 {
    let day_span = (last_date - first_date) as f64;
    if day_span > 0.0 && n_observations > 0 {
        let years = day_span / 365.25;
        let obs_per_year = n_observations as f64 / years;
        obs_per_year.sqrt()
    } else {
        252.0_f64.sqrt() // fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    // --- Skewness ---

    #[test]
    fn skewness_symmetric_returns_near_zero() {
        let returns = vec![-0.02, -0.01, 0.0, 0.01, 0.02];
        assert!(skewness(&returns).abs() < 0.01);
    }

    #[test]
    fn skewness_right_skewed_positive() {
        // Bulk of returns near zero, one large positive outlier
        let mut returns = vec![0.01; 100];
        returns.push(0.50);
        assert!(skewness(&returns) > 0.5);
    }

    #[test]
    fn skewness_too_few_returns_zero() {
        assert_eq!(skewness(&[0.01, 0.02]), 0.0);
        assert_eq!(skewness(&[]), 0.0);
    }

    // --- Excess kurtosis ---

    #[test]
    fn kurtosis_normal_like_near_zero() {
        // Large sample of uniform-ish returns: kurtosis ≈ -1.2 (platykurtic)
        let returns: Vec<f64> = (0..1000).map(|i| (i as f64 / 1000.0) - 0.5).collect();
        let k = excess_kurtosis(&returns);
        assert!(k < 0.0, "uniform should be platykurtic, got {k}");
    }

    #[test]
    fn kurtosis_heavy_tailed_positive() {
        // Mostly zero, with fat-tail outliers → leptokurtic
        let mut returns = vec![0.0; 100];
        returns.push(1.0);
        returns.push(-1.0);
        let k = excess_kurtosis(&returns);
        assert!(k > 5.0, "fat-tailed should be leptokurtic, got {k}");
    }

    #[test]
    fn kurtosis_too_few_returns_zero() {
        assert_eq!(excess_kurtosis(&[0.01, 0.02, 0.03]), 0.0);
    }

    // --- Sharpe std error ---

    #[test]
    fn sharpe_std_err_basic() {
        // SR=1.0, n=252, skew=0, excess_kurt=0
        // numerator = 1 - 0*1 + (0+2)/4 * 1^2 = 1.5
        // SE = sqrt(1.5 / 251) ≈ 0.07731
        let se = sharpe_std_err(1.0, 252, 0.0, 0.0);
        assert!(approx_eq(se, (1.5 / 251.0_f64).sqrt(), 0.001));
    }

    #[test]
    fn sharpe_std_err_skew_increases() {
        // Negative skew + positive SR increases SE
        let se_no_skew = sharpe_std_err(1.5, 100, 0.0, 0.0);
        let se_neg_skew = sharpe_std_err(1.5, 100, -1.0, 0.0);
        assert!(se_neg_skew > se_no_skew);
    }

    #[test]
    fn sharpe_std_err_too_few() {
        assert_eq!(sharpe_std_err(1.0, 1, 0.0, 0.0), f64::MAX);
        assert_eq!(sharpe_std_err(1.0, 0, 0.0, 0.0), f64::MAX);
    }

    // --- Probabilistic Sharpe Ratio ---

    #[test]
    fn psr_high_sharpe_near_one() {
        // SR=2.0, n=500, normal-ish → PSR should be very high
        let psr = probabilistic_sharpe(2.0, 0.0, 500, 0.0, 0.0);
        assert!(psr > 0.99, "high Sharpe with many obs should give PSR > 0.99, got {psr}");
    }

    #[test]
    fn psr_zero_sharpe_near_half() {
        // SR=0.0, benchmark=0 → z=0 → PSR ≈ 0.5
        let psr = probabilistic_sharpe(0.0, 0.0, 100, 0.0, 0.0);
        assert!(approx_eq(psr, 0.5, 0.05), "zero SR should give PSR ≈ 0.5, got {psr}");
    }

    #[test]
    fn psr_negative_sharpe_below_half() {
        let psr = probabilistic_sharpe(-1.0, 0.0, 200, 0.0, 0.0);
        assert!(psr < 0.05, "negative Sharpe should give low PSR, got {psr}");
    }

    // --- Deflated Sharpe Ratio ---

    #[test]
    fn dsr_single_trial_equals_psr() {
        let sr = 0.15;
        let psr = probabilistic_sharpe(sr, 0.0, 200, 0.2, 1.0);
        let dsr = deflated_sharpe(sr, 200, 0.2, 1.0, 1);
        assert!(approx_eq(dsr, psr, 1e-10));
    }

    #[test]
    fn dsr_many_trials_lower_than_psr() {
        // Realistic per-trade SR: mean/std ≈ 0.15 for a decent strategy
        // With 100 trials and 50 observations, SR_0 ≈ 0.43 > 0.15 → DSR well below PSR
        let sr = 0.15;
        let psr = probabilistic_sharpe(sr, 0.0, 50, 0.0, 0.0);
        let dsr = deflated_sharpe(sr, 50, 0.0, 0.0, 100);
        assert!(dsr < psr, "DSR({dsr}) should be < PSR({psr}) with 100 trials");
    }

    #[test]
    fn dsr_thousand_trials_stricter_than_hundred() {
        // More trials → higher SR_0 → lower DSR
        let dsr_100 = deflated_sharpe(0.15, 200, 0.0, 0.0, 100);
        let dsr_1000 = deflated_sharpe(0.15, 200, 0.0, 0.0, 1000);
        assert!(dsr_1000 < dsr_100, "DSR({dsr_1000}) should be < DSR({dsr_100}) with more trials");
    }

    // --- norm_cdf ---

    #[test]
    fn norm_cdf_known_values() {
        assert!(approx_eq(norm_cdf(0.0), 0.5, 1e-6));
        assert!(approx_eq(norm_cdf(1.96), 0.975, 1e-3));
        assert!(approx_eq(norm_cdf(-1.96), 0.025, 1e-3));
    }

    #[test]
    fn norm_cdf_extreme_values() {
        assert_eq!(norm_cdf(-10.0), 0.0);
        assert_eq!(norm_cdf(10.0), 1.0);
    }

    // --- Existing metrics regression ---

    #[test]
    fn profit_factor_all_winners() {
        assert_eq!(profit_factor(1000.0, 0.0), f64::MAX);
    }

    #[test]
    fn profit_factor_all_losers() {
        assert_eq!(profit_factor(0.0, 500.0), 0.0);
    }

    #[test]
    fn max_drawdown_monotonic_up() {
        assert_eq!(max_drawdown(&[100.0, 110.0, 120.0, 130.0]), 0.0);
    }

    #[test]
    fn max_drawdown_50_percent() {
        let eq = vec![100.0, 50.0, 75.0];
        assert!(approx_eq(max_drawdown(&eq), 0.5, 1e-10));
    }

    #[test]
    fn cagr_double_in_one_year() {
        assert!(approx_eq(cagr(100.0, 200.0, 1.0), 1.0, 1e-10));
    }

    #[test]
    fn equity_to_returns_simple() {
        let eq = vec![100.0, 110.0, 99.0];
        let r = equity_to_returns(&eq);
        assert_eq!(r.len(), 2);
        assert!(approx_eq(r[0], 0.1, 1e-10));
        assert!(approx_eq(r[1], -0.1, 1e-10));
    }
}
