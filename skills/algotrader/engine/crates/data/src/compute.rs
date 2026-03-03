//! Runtime indicator computation from raw OHLCV data.
//!
//! Computes column-local indicators (SMA, ATR, rolling returns, etc.) and
//! cross-sectional indicators (RS percentile rank). Column-by-column processing
//! with temporary buffers keeps inner loops tight for auto-vectorization.
//!
//! Kept in cache (not here): multi-pass VCP and flag pattern detection.

use engine_types::WideMatrix;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Column-local indicators computed from OHLCV.
///
/// Returns `[sma10, sma20, vol_sma20, atr14, ret21, ret63, ret126, pct10d,
///            dist52w, consol_high, consec_green]`.
pub fn compute_column_local(
    open: &WideMatrix,
    high: &WideMatrix,
    low: &WideMatrix,
    close: &WideMatrix,
    volume: &WideMatrix,
) -> [WideMatrix; 11] {
    let n_rows = close.n_rows();
    let n_cols = close.n_cols();

    let sma10 = rolling_mean(close, 10);
    let sma20 = rolling_mean(close, 20);
    let vol_sma20 = rolling_mean(volume, 20);
    let atr14 = wilder_atr(high, low, close, 14);
    let ret21 = pct_change(close, 21);
    let ret63 = pct_change(close, 63);
    let ret126 = pct_change(close, 126);
    let pct10d = pct_change(close, 10);
    let dist52w = dist_from_rolling_high(high, close, 252);
    let consol_high = consol_high(high);
    let consec_green = consec_green(open, close, n_rows, n_cols);

    [sma10, sma20, vol_sma20, atr14, ret21, ret63, ret126, pct10d, dist52w, consol_high, consec_green]
}

/// Cross-sectional percentile rank of rolling returns.
///
/// For each row, ranks all non-NaN tickers by return value and assigns
/// a percentile in [0.0, 1.0] where 1.0 = highest return (top percentile).
pub fn cross_sectional_pctrank(returns: &WideMatrix) -> WideMatrix {
    let n_rows = returns.n_rows();
    let n_cols = returns.n_cols();
    let mut out = vec![f32::NAN; n_rows * n_cols];

    // Scratch buffers reused across rows
    let mut indices: Vec<usize> = Vec::with_capacity(n_cols);

    for row in 0..n_rows {
        indices.clear();

        // Collect columns with valid return values
        for col in 0..n_cols {
            let v = returns.get(row, col);
            if !v.is_nan() {
                indices.push(col);
            }
        }

        let count = indices.len();
        if count == 0 {
            continue;
        }

        // Sort by return value (ascending)
        indices.sort_unstable_by(|&a, &b| {
            let va = returns.get(row, a);
            let vb = returns.get(row, b);
            va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Assign percentile rank: rank / (count - 1) for count > 1
        // With ties handled by average rank (matching pandas pct=True behavior)
        let denom = if count > 1 { (count - 1) as f32 } else { 1.0 };
        let mut i = 0;
        while i < count {
            // Find the run of tied values
            let val = returns.get(row, indices[i]);
            let mut j = i + 1;
            while j < count && returns.get(row, indices[j]) == val {
                j += 1;
            }
            // Average rank for the tie group
            let avg_rank = (i + j - 1) as f32 / 2.0;
            let pct = avg_rank / denom;
            for k in i..j {
                out[row * n_cols + indices[k]] = pct;
            }
            i = j;
        }
    }

    WideMatrix::new(out, n_rows, n_cols)
}

/// Compute RS percentile ranks for 1m (21-day), 3m (63-day), 6m (126-day).
pub fn compute_rs_pctrank(
    ret21: &WideMatrix,
    ret63: &WideMatrix,
    ret126: &WideMatrix,
) -> [WideMatrix; 3] {
    let rs_1m = cross_sectional_pctrank(ret21);
    let rs_3m = cross_sectional_pctrank(ret63);
    let rs_6m = cross_sectional_pctrank(ret126);
    [rs_1m, rs_3m, rs_6m]
}

// ---------------------------------------------------------------------------
// Rolling mean (ring buffer, single forward pass per column)
// ---------------------------------------------------------------------------

pub fn rolling_mean(src: &WideMatrix, period: usize) -> WideMatrix {
    let n_rows = src.n_rows();
    let n_cols = src.n_cols();
    let mut out = vec![f32::NAN; n_rows * n_cols];

    for col in 0..n_cols {
        let mut ring = vec![0.0f32; period];
        let mut sum = 0.0f64;
        let mut filled = 0usize;

        for row in 0..n_rows {
            let val = src.get(row, col);
            if val.is_nan() {
                // NaN resets the window
                sum = 0.0;
                filled = 0;
                ring.iter_mut().for_each(|x| *x = 0.0);
                continue;
            }
            let slot = row % period;
            sum -= ring[slot] as f64;
            ring[slot] = val;
            sum += val as f64;
            if filled < period {
                filled += 1;
            }
            if filled == period {
                out[row * n_cols + col] = (sum / period as f64) as f32;
            }
        }
    }

    WideMatrix::new(out, n_rows, n_cols)
}

// ---------------------------------------------------------------------------
// Wilder ATR (Wilder smoothing = 1/period alpha)
// ---------------------------------------------------------------------------

pub fn wilder_atr(
    high: &WideMatrix,
    low: &WideMatrix,
    close: &WideMatrix,
    period: usize,
) -> WideMatrix {
    let n_rows = high.n_rows();
    let n_cols = high.n_cols();
    let mut out = vec![f32::NAN; n_rows * n_cols];
    let alpha = 1.0f64 / period as f64;

    for col in 0..n_cols {
        // Warmup: simple average of first `period` TRs
        let mut warmup_sum = 0.0f64;
        let mut warmup_count = 0usize;
        let mut atr = f64::NAN;
        let mut prev_close = f64::NAN;

        for row in 0..n_rows {
            let h = high.get(row, col) as f64;
            let l = low.get(row, col) as f64;
            let c = close.get(row, col) as f64;

            if h.is_nan() || l.is_nan() || c.is_nan() {
                prev_close = f64::NAN;
                atr = f64::NAN;
                warmup_sum = 0.0;
                warmup_count = 0;
                continue;
            }

            let tr = if prev_close.is_nan() {
                h - l
            } else {
                let hl = h - l;
                let hc = (h - prev_close).abs();
                let lc = (l - prev_close).abs();
                hl.max(hc).max(lc)
            };

            if atr.is_nan() {
                // Still warming up
                warmup_sum += tr;
                warmup_count += 1;
                if warmup_count == period {
                    atr = warmup_sum / period as f64;
                    out[row * n_cols + col] = atr as f32;
                }
            } else {
                // Wilder smoothing
                atr = atr * (1.0 - alpha) + tr * alpha;
                out[row * n_cols + col] = atr as f32;
            }

            prev_close = c;
        }
    }

    WideMatrix::new(out, n_rows, n_cols)
}

// ---------------------------------------------------------------------------
// Percentage change over N periods
// ---------------------------------------------------------------------------

pub fn pct_change(src: &WideMatrix, period: usize) -> WideMatrix {
    let n_rows = src.n_rows();
    let n_cols = src.n_cols();
    let mut out = vec![f32::NAN; n_rows * n_cols];

    for col in 0..n_cols {
        for row in period..n_rows {
            let cur = src.get(row, col);
            let prev = src.get(row - period, col);
            if !cur.is_nan() && !prev.is_nan() && prev > 0.0 {
                out[row * n_cols + col] = cur / prev - 1.0;
            }
        }
    }

    WideMatrix::new(out, n_rows, n_cols)
}

// ---------------------------------------------------------------------------
// Rolling maximum (monotonic deque, O(n) amortized)
// ---------------------------------------------------------------------------

pub fn rolling_max(src: &WideMatrix, period: usize) -> WideMatrix {
    let n_rows = src.n_rows();
    let n_cols = src.n_cols();
    let mut out = vec![f32::NAN; n_rows * n_cols];

    for col in 0..n_cols {
        // Deque stores row indices, values are non-increasing
        let mut deque: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        let mut filled: usize = 0; // valid values since last NaN reset

        for row in 0..n_rows {
            let val = src.get(row, col);
            if val.is_nan() {
                deque.clear();
                filled = 0;
                continue;
            }

            filled += 1;

            // Remove elements outside the window
            while deque.front().is_some_and(|&r| row >= r + period) {
                deque.pop_front();
            }

            // Maintain non-increasing order: remove smaller trailing elements
            while deque
                .back()
                .is_some_and(|&r| src.get(r, col) <= val || src.get(r, col).is_nan())
            {
                deque.pop_back();
            }

            deque.push_back(row);

            // Only output once we have a full window of valid values
            if filled >= period {
                out[row * n_cols + col] = src.get(*deque.front().unwrap(), col);
            }
        }
    }

    WideMatrix::new(out, n_rows, n_cols)
}

// ---------------------------------------------------------------------------
// Distance from 52-week (252-bar) rolling high
// ---------------------------------------------------------------------------

pub fn dist_from_rolling_high(
    high: &WideMatrix,
    close: &WideMatrix,
    period: usize,
) -> WideMatrix {
    let n_rows = high.n_rows();
    let n_cols = high.n_cols();
    let rolling_max_high = rolling_max(high, period);
    let mut out = vec![f32::NAN; n_rows * n_cols];

    for row in 0..n_rows {
        for col in 0..n_cols {
            let max_h = rolling_max_high.get(row, col);
            let c = close.get(row, col);
            if !max_h.is_nan() && !c.is_nan() && max_h > 0.0 {
                out[row * n_cols + col] = (max_h - c) / max_h;
            }
        }
    }

    WideMatrix::new(out, n_rows, n_cols)
}

// ---------------------------------------------------------------------------
// Consolidation high: high.shift(1).rolling(40).max()
// ---------------------------------------------------------------------------

pub fn consol_high(high: &WideMatrix) -> WideMatrix {
    let n_rows = high.n_rows();
    let n_cols = high.n_cols();

    // Shift high by 1: row r gets value from row r-1
    let mut shifted = vec![f32::NAN; n_rows * n_cols];
    for row in 1..n_rows {
        for col in 0..n_cols {
            shifted[row * n_cols + col] = high.get(row - 1, col);
        }
    }
    let shifted_m = WideMatrix::new(shifted, n_rows, n_cols);

    rolling_max(&shifted_m, 40)
}

// ---------------------------------------------------------------------------
// Consecutive green bars (close > open streak)
// ---------------------------------------------------------------------------

pub fn consec_green(open: &WideMatrix, close: &WideMatrix, n_rows: usize, n_cols: usize) -> WideMatrix {
    let mut out = vec![0.0f32; n_rows * n_cols];

    for col in 0..n_cols {
        let mut streak = 0.0f32;
        for row in 0..n_rows {
            let o = open.get(row, col);
            let c = close.get(row, col);
            if o.is_nan() || c.is_nan() {
                streak = 0.0;
            } else if c > o {
                streak += 1.0;
            } else {
                streak = 0.0;
            }
            out[row * n_cols + col] = streak;
        }
    }

    WideMatrix::new(out, n_rows, n_cols)
}

// ---------------------------------------------------------------------------
// Derived indicators for filter fusion
// ---------------------------------------------------------------------------

/// ADR% = ATR14 / Close. NaN if either input is NaN or close <= 0.
pub fn adr_pct(atr14: &WideMatrix, close: &WideMatrix) -> WideMatrix {
    let n_rows = atr14.n_rows();
    let n_cols = atr14.n_cols();
    let mut out = vec![f32::NAN; n_rows * n_cols];

    for row in 0..n_rows {
        for col in 0..n_cols {
            let a = atr14.get(row, col);
            let c = close.get(row, col);
            if !a.is_nan() && !c.is_nan() && c > 0.0 {
                out[row * n_cols + col] = a / c;
            }
        }
    }

    WideMatrix::new(out, n_rows, n_cols)
}

/// Extension in ATR units = (Close - ConsolHigh) / ATR14.
/// When ConsolHigh or ATR14 is NaN (or ATR <= 0), returns NEG_INFINITY so that
/// a `<= threshold` fusable check always passes (matching the block's NaN-passthrough behavior).
pub fn extension_atr(close: &WideMatrix, consol_high: &WideMatrix, atr14: &WideMatrix) -> WideMatrix {
    let n_rows = close.n_rows();
    let n_cols = close.n_cols();
    let mut out = vec![f32::NEG_INFINITY; n_rows * n_cols];

    for row in 0..n_rows {
        for col in 0..n_cols {
            let c = close.get(row, col);
            let ch = consol_high.get(row, col);
            let a = atr14.get(row, col);
            if !ch.is_nan() && !a.is_nan() && a > 0.0 {
                out[row * n_cols + col] = (c - ch) / a;
            }
            // else: stays NEG_INFINITY → passes Le threshold check
        }
    }

    WideMatrix::new(out, n_rows, n_cols)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a 1-column WideMatrix from a slice.
    fn col(vals: &[f32]) -> WideMatrix {
        WideMatrix::new(vals.to_vec(), vals.len(), 1)
    }

    const EPS: f32 = 1e-4;

    // -- Fix #3: rolling_max NaN-reset must require `period` valid values -----

    #[test]
    fn rolling_max_basic() {
        // [1, 3, 2, 5, 4] with period=3 → [NaN, NaN, 3, 5, 5]
        let src = col(&[1.0, 3.0, 2.0, 5.0, 4.0]);
        let out = rolling_max(&src, 3);
        assert!(out.get(0, 0).is_nan());
        assert!(out.get(1, 0).is_nan());
        assert!((out.get(2, 0) - 3.0).abs() < EPS);
        assert!((out.get(3, 0) - 5.0).abs() < EPS);
        assert!((out.get(4, 0) - 5.0).abs() < EPS);
    }

    #[test]
    fn rolling_max_nan_reset_suppresses_early_output() {
        // Period=3. After a NaN at row 3, need 3 more valid values before emitting.
        // [1, 2, 3, NaN, 10, 20, 30, 40]
        //  0  1  2   3    4   5   6   7
        // Pre-NaN: output at rows 2 (=3), no output rows 0,1
        // Post-NaN: rows 4,5 should be NaN (only 1 and 2 valid values),
        //           row 6 should emit 30 (3 valid), row 7 should emit 40
        let src = col(&[1.0, 2.0, 3.0, f32::NAN, 10.0, 20.0, 30.0, 40.0]);
        let out = rolling_max(&src, 3);

        // Before NaN
        assert!(out.get(0, 0).is_nan());
        assert!(out.get(1, 0).is_nan());
        assert!((out.get(2, 0) - 3.0).abs() < EPS);

        // NaN row itself
        assert!(out.get(3, 0).is_nan());

        // After NaN: must NOT emit until 3 valid values seen
        assert!(out.get(4, 0).is_nan(), "only 1 valid after reset, should be NaN");
        assert!(out.get(5, 0).is_nan(), "only 2 valid after reset, should be NaN");

        // 3 valid values accumulated → emit
        assert!((out.get(6, 0) - 30.0).abs() < EPS, "3 valid → max(10,20,30)=30");
        assert!((out.get(7, 0) - 40.0).abs() < EPS, "4 valid → max(20,30,40)=40");
    }

    #[test]
    fn rolling_max_all_nan_produces_all_nan() {
        let src = col(&[f32::NAN; 5]);
        let out = rolling_max(&src, 3);
        for r in 0..5 {
            assert!(out.get(r, 0).is_nan());
        }
    }

    // -- rolling_mean basic + NaN reset (regression guard) --------------------

    #[test]
    fn rolling_mean_basic() {
        let src = col(&[2.0, 4.0, 6.0, 8.0, 10.0]);
        let out = rolling_mean(&src, 3);
        assert!(out.get(0, 0).is_nan());
        assert!(out.get(1, 0).is_nan());
        assert!((out.get(2, 0) - 4.0).abs() < EPS); // (2+4+6)/3
        assert!((out.get(3, 0) - 6.0).abs() < EPS); // (4+6+8)/3
        assert!((out.get(4, 0) - 8.0).abs() < EPS); // (6+8+10)/3
    }

    #[test]
    fn rolling_mean_nan_reset_requires_period_values() {
        // [1, 2, 3, NaN, 10, 20, 30]  period=3
        let src = col(&[1.0, 2.0, 3.0, f32::NAN, 10.0, 20.0, 30.0]);
        let out = rolling_mean(&src, 3);
        assert!((out.get(2, 0) - 2.0).abs() < EPS); // (1+2+3)/3
        assert!(out.get(3, 0).is_nan()); // NaN row
        assert!(out.get(4, 0).is_nan(), "only 1 valid after reset");
        assert!(out.get(5, 0).is_nan(), "only 2 valid after reset");
        assert!((out.get(6, 0) - 20.0).abs() < EPS); // (10+20+30)/3
    }

    // -- consec_green ---------------------------------------------------------

    #[test]
    fn consec_green_counts_streaks() {
        let open  = col(&[10.0, 10.0, 10.0, 10.0, 10.0]);
        let close = col(&[11.0, 12.0,  9.0, 11.0, 13.0]);
        let out = consec_green(&open, &close, 5, 1);
        assert!((out.get(0, 0) - 1.0).abs() < EPS);
        assert!((out.get(1, 0) - 2.0).abs() < EPS);
        assert!((out.get(2, 0) - 0.0).abs() < EPS); // red
        assert!((out.get(3, 0) - 1.0).abs() < EPS);
        assert!((out.get(4, 0) - 2.0).abs() < EPS);
    }

    #[test]
    fn consec_green_nan_resets_streak() {
        let open  = col(&[10.0, 10.0, f32::NAN, 10.0, 10.0]);
        let close = col(&[11.0, 12.0, f32::NAN, 11.0, 12.0]);
        let out = consec_green(&open, &close, 5, 1);
        assert!((out.get(1, 0) - 2.0).abs() < EPS);
        assert!((out.get(2, 0) - 0.0).abs() < EPS);
        assert!((out.get(3, 0) - 1.0).abs() < EPS);
    }

    // -- pct_change -----------------------------------------------------------

    #[test]
    fn pct_change_basic() {
        let src = col(&[100.0, 110.0, 105.0]);
        let out = pct_change(&src, 1);
        assert!(out.get(0, 0).is_nan());
        assert!((out.get(1, 0) - 0.1).abs() < EPS);
        let expected = 105.0 / 110.0 - 1.0;
        assert!((out.get(2, 0) - expected).abs() < EPS);
    }

    #[test]
    fn pct_change_zero_prev_produces_nan() {
        let src = col(&[0.0, 10.0]);
        let out = pct_change(&src, 1);
        assert!(out.get(1, 0).is_nan(), "division by zero → NaN");
    }

    // -- wilder_atr -----------------------------------------------------------

    // -- cross_sectional_pctrank -----------------------------------------------

    #[test]
    fn pctrank_basic_4_tickers() {
        // 1 row, 4 cols: values [10, 30, 20, 40]
        // Sorted: 10(col0), 20(col2), 30(col1), 40(col3)
        // Ranks:  0/3=0.0,  2/3=0.667, 1/3=0.333, 3/3=1.0
        let src = WideMatrix::new(vec![10.0, 30.0, 20.0, 40.0], 1, 4);
        let out = cross_sectional_pctrank(&src);
        assert!((out.get(0, 0) - 0.0).abs() < EPS);
        assert!((out.get(0, 1) - 2.0 / 3.0).abs() < EPS);
        assert!((out.get(0, 2) - 1.0 / 3.0).abs() < EPS);
        assert!((out.get(0, 3) - 1.0).abs() < EPS);
    }

    #[test]
    fn pctrank_with_nan() {
        // 1 row, 4 cols: [NaN, 30, 10, 20]
        // Valid: 10(col2), 20(col3), 30(col1) → ranks 0/2, 1/2, 2/2
        let src = WideMatrix::new(vec![f32::NAN, 30.0, 10.0, 20.0], 1, 4);
        let out = cross_sectional_pctrank(&src);
        assert!(out.get(0, 0).is_nan());
        assert!((out.get(0, 1) - 1.0).abs() < EPS);
        assert!((out.get(0, 2) - 0.0).abs() < EPS);
        assert!((out.get(0, 3) - 0.5).abs() < EPS);
    }

    #[test]
    fn pctrank_ties_get_average_rank() {
        // 1 row, 4 cols: [10, 10, 20, 20]
        // Sorted: 10(0), 10(1), 20(2), 20(3)
        // Tie group [0,1]: avg_rank = 0.5, pct = 0.5/3 = 0.1667
        // Tie group [2,3]: avg_rank = 2.5, pct = 2.5/3 = 0.8333
        let src = WideMatrix::new(vec![10.0, 10.0, 20.0, 20.0], 1, 4);
        let out = cross_sectional_pctrank(&src);
        assert!((out.get(0, 0) - 0.5 / 3.0).abs() < EPS);
        assert!((out.get(0, 1) - 0.5 / 3.0).abs() < EPS);
        assert!((out.get(0, 2) - 2.5 / 3.0).abs() < EPS);
        assert!((out.get(0, 3) - 2.5 / 3.0).abs() < EPS);
    }

    #[test]
    fn pctrank_single_value() {
        // 1 row, 1 col: single value → rank 0/0, denom=1 → 0.0
        let src = WideMatrix::new(vec![42.0], 1, 1);
        let out = cross_sectional_pctrank(&src);
        assert!((out.get(0, 0) - 0.0).abs() < EPS);
    }

    #[test]
    fn pctrank_all_nan_row() {
        let src = WideMatrix::new(vec![f32::NAN; 4], 1, 4);
        let out = cross_sectional_pctrank(&src);
        for c in 0..4 {
            assert!(out.get(0, c).is_nan());
        }
    }

    // -- wilder_atr -----------------------------------------------------------

    #[test]
    fn wilder_atr_warmup_then_smooth() {
        // Period=3, constant TR=2.0 → ATR stays at 2.0
        let h = col(&[12.0, 12.0, 12.0, 12.0, 12.0]);
        let l = col(&[10.0, 10.0, 10.0, 10.0, 10.0]);
        let c = col(&[11.0, 11.0, 11.0, 11.0, 11.0]);
        let out = wilder_atr(&h, &l, &c, 3);
        assert!(out.get(0, 0).is_nan());
        assert!(out.get(1, 0).is_nan());
        assert!((out.get(2, 0) - 2.0).abs() < EPS); // simple avg of 3 TRs
        assert!((out.get(3, 0) - 2.0).abs() < EPS); // constant → stays 2.0
    }
}
