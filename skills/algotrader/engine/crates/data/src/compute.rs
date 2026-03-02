//! Runtime indicator computation from raw OHLCV data.
//!
//! Computes 10 indicators that are trivially single-pass forward operations
//! over per-column time series. These don't need pre-cached parquet files.
//! Column-by-column processing with temporary buffers keeps inner loops tight
//! so the compiler can auto-vectorize them.
//!
//! Kept in cache (not here): cross-sectional RS rank, multi-pass VCP, flag
//! pattern detection.

use engine_types::WideMatrix;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Fill all 10 runtime-computed indicator slots from the OHLCV matrices.
///
/// Returns `[sma10, sma20, vol_sma20, atr14, ret63, ret126, pct10d, dist52w,
///            consol_high, consec_green]` in that order (matching the order
/// used by `compute_runtime_indicators` in `loader.rs`).
pub fn compute_all(
    open: &WideMatrix,
    high: &WideMatrix,
    low: &WideMatrix,
    close: &WideMatrix,
    volume: &WideMatrix,
) -> [WideMatrix; 10] {
    let n_rows = close.n_rows();
    let n_cols = close.n_cols();

    let sma10 = rolling_mean(close, 10);
    let sma20 = rolling_mean(close, 20);
    let vol_sma20 = rolling_mean(volume, 20);
    let atr14 = wilder_atr(high, low, close, 14);
    let ret63 = pct_change(close, 63);
    let ret126 = pct_change(close, 126);
    let pct10d = pct_change(close, 10);
    let dist52w = dist_from_rolling_high(high, close, 252);
    let consol_high = consol_high(high);
    let consec_green = consec_green(open, close, n_rows, n_cols);

    [sma10, sma20, vol_sma20, atr14, ret63, ret126, pct10d, dist52w, consol_high, consec_green]
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
