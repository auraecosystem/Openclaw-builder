//! Pearson correlation between strategy equity curves.
//!
//! Strategies run over the same date range but trade at different times, so we
//! project each trade-exit equity curve onto a shared daily-row grid (carry-
//! forward) before computing correlation. This gives sensible zero correlation
//! for strategies with completely disjoint activity, rather than NaN.

use super::report::EquityCurve;

/// Project a trade-exit equity curve onto a dense daily grid of length `n_rows`.
///
/// Equity starts at `init_cash` and carries forward from each exit. Rows before
/// the first exit remain at `init_cash`.
pub fn to_daily_equity(curve: &EquityCurve, n_rows: usize, init_cash: f64) -> Vec<f64> {
    let mut daily = vec![init_cash; n_rows];

    // Stamp equity at each exit row. When multiple exits share a row,
    // the last (highest-index) write is the most inclusive cumulative value.
    for (i, &row) in curve.exit_rows.iter().enumerate() {
        if row < n_rows {
            daily[row] = curve.values[i + 1];
        }
    }

    // Forward-fill: carry the last stamped/inherited value forward.
    // Use a sorted set for O(1) lookup of exit rows.
    let exit_set: std::collections::HashSet<usize> =
        curve.exit_rows.iter().copied().collect();
    for r in 1..n_rows {
        if !exit_set.contains(&r) {
            daily[r] = daily[r - 1];
        }
    }
    daily
}

/// Pearson correlation between two equal-length slices.
/// Returns 0.0 if either slice has zero variance.
pub fn pearson(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "pearson: slices must have equal length");
    let n = a.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let mean_a = a.iter().sum::<f64>() / n;
    let mean_b = b.iter().sum::<f64>() / n;

    let (cov, var_a, var_b) = a.iter().zip(b.iter()).fold(
        (0.0_f64, 0.0_f64, 0.0_f64),
        |(cov, va, vb), (&ai, &bi)| {
            let da = ai - mean_a;
            let db = bi - mean_b;
            (cov + da * db, va + da * da, vb + db * db)
        },
    );

    let denom = var_a.sqrt() * var_b.sqrt();
    if denom < 1e-12 { 0.0 } else { cov / denom }
}

/// Compute the N×N Pearson correlation matrix from a list of daily equity curves.
///
/// Correlation is computed on daily log-returns (not equity levels).
/// Diagonal entries are 1.0.
pub fn correlation_matrix(daily_curves: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = daily_curves.len();

    let returns: Vec<Vec<f64>> = daily_curves
        .iter()
        .map(|eq| {
            (1..eq.len())
                .map(|i| {
                    if eq[i - 1] > 0.0 {
                        (eq[i] / eq[i - 1]).ln()
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect();

    let mut matrix = vec![vec![0.0_f64; n]; n];
    for i in 0..n {
        matrix[i][i] = 1.0;
        for j in (i + 1)..n {
            let r = pearson(&returns[i], &returns[j]);
            matrix[i][j] = r;
            matrix[j][i] = r;
        }
    }
    matrix
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-6;

    // -- Fix #2 (critical): pearson must reject unequal-length slices ---------

    #[test]
    #[should_panic(expected = "equal length")]
    fn pearson_panics_on_mismatched_lengths() {
        pearson(&[1.0, 2.0, 3.0], &[1.0, 2.0]);
    }

    // -- pearson correctness --------------------------------------------------

    #[test]
    fn pearson_perfect_positive() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![2.0, 4.0, 6.0, 8.0, 10.0]; // b = 2*a
        let r = pearson(&a, &b);
        assert!((r - 1.0).abs() < EPS, "perfectly correlated → 1.0, got {r}");
    }

    #[test]
    fn pearson_perfect_negative() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![10.0, 8.0, 6.0, 4.0, 2.0]; // b = 12 - 2*a
        let r = pearson(&a, &b);
        assert!((r - (-1.0)).abs() < EPS, "perfectly anti-correlated → -1.0, got {r}");
    }

    #[test]
    fn pearson_zero_variance_returns_zero() {
        let a = vec![5.0, 5.0, 5.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(pearson(&a, &b), 0.0);
    }

    #[test]
    fn pearson_single_element_returns_zero() {
        assert_eq!(pearson(&[1.0], &[2.0]), 0.0);
    }

    // -- to_daily_equity: forward-fill correctness (Fix #7 / #8) --------------

    #[test]
    fn to_daily_equity_forward_fills_between_exits() {
        // 10 rows, init_cash=100. Exits at rows 2 and 7.
        let curve = EquityCurve {
            values: vec![100.0, 110.0, 130.0], // init, after exit 0, after exit 1
            exit_rows: vec![2, 7],
        };
        let daily = to_daily_equity(&curve, 10, 100.0);

        // Rows 0-1: init_cash (before first exit)
        assert_eq!(daily[0], 100.0);
        assert_eq!(daily[1], 100.0);
        // Row 2: first exit → 110
        assert_eq!(daily[2], 110.0);
        // Rows 3-6: carry forward 110
        assert_eq!(daily[3], 110.0);
        assert_eq!(daily[6], 110.0);
        // Row 7: second exit → 130
        assert_eq!(daily[7], 130.0);
        // Rows 8-9: carry forward 130
        assert_eq!(daily[8], 130.0);
        assert_eq!(daily[9], 130.0);
    }

    #[test]
    fn to_daily_equity_multiple_exits_same_row() {
        // Two exits on row 3 — last value (cumulative) wins
        let curve = EquityCurve {
            values: vec![100.0, 105.0, 115.0],
            exit_rows: vec![3, 3],
        };
        let daily = to_daily_equity(&curve, 6, 100.0);
        // Row 3 should have the last cumulative value (115.0)
        assert_eq!(daily[3], 115.0);
        // Forward-filled
        assert_eq!(daily[5], 115.0);
    }

    #[test]
    fn to_daily_equity_no_exits() {
        let curve = EquityCurve {
            values: vec![100.0],
            exit_rows: vec![],
        };
        let daily = to_daily_equity(&curve, 5, 100.0);
        for v in &daily {
            assert_eq!(*v, 100.0);
        }
    }

    // -- correlation_matrix ---------------------------------------------------

    #[test]
    fn correlation_matrix_diagonal_is_one() {
        let curves = vec![
            vec![100.0, 101.0, 102.0, 103.0],
            vec![100.0, 99.0, 98.0, 97.0],
        ];
        let m = correlation_matrix(&curves);
        assert_eq!(m.len(), 2);
        assert!((m[0][0] - 1.0).abs() < EPS);
        assert!((m[1][1] - 1.0).abs() < EPS);
    }

    #[test]
    fn correlation_matrix_symmetric() {
        let curves = vec![
            vec![100.0, 102.0, 101.0, 105.0],
            vec![100.0, 99.0, 101.0, 98.0],
        ];
        let m = correlation_matrix(&curves);
        assert!((m[0][1] - m[1][0]).abs() < EPS);
    }

    #[test]
    fn correlation_matrix_identical_curves_are_one() {
        let curve = vec![100.0, 105.0, 103.0, 110.0, 108.0];
        let m = correlation_matrix(&[curve.clone(), curve]);
        assert!((m[0][1] - 1.0).abs() < EPS, "identical curves → corr=1.0");
    }
}
