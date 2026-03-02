//! OHLCV resampling: aggregate 5m bars into coarser timeframes.
//!
//! Aggregation rules (standard OHLCV):
//! - Open  = first bar's open in the group
//! - High  = max high in the group
//! - Low   = min low in the group
//! - Close = last bar's close in the group
//! - Volume = sum of volumes in the group
//!
//! The output matrices have `ceil(n_5m / factor)` rows and `n_cols` columns,
//! in the same row-major WideMatrix layout as the input.

use engine_types::WideMatrix;

/// Resample `n_5m × n_cols` OHLCV matrices by aggregating `factor` consecutive
/// 5m bars into one coarser bar.
///
/// Returns `[open, high, low, close, volume]` matrices with
/// `ceil(n_5m / factor)` rows.
///
/// Bars with all-NaN values (e.g. outside trading hours) are skipped:
/// the aggregation only uses valid (non-NaN) bars within each group.
/// If an entire group is NaN, the output row is NaN.
pub fn resample_ohlcv(
    open_5m: &WideMatrix,
    high_5m: &WideMatrix,
    low_5m: &WideMatrix,
    close_5m: &WideMatrix,
    volume_5m: &WideMatrix,
    factor: usize,
) -> [WideMatrix; 5] {
    assert!(factor >= 1);
    let n_5m = open_5m.n_rows();
    let n_cols = open_5m.n_cols();
    let n_out = (n_5m + factor - 1) / factor; // ceil division

    let mut out_open = vec![f32::NAN; n_out * n_cols];
    let mut out_high = vec![f32::NAN; n_out * n_cols];
    let mut out_low = vec![f32::NAN; n_out * n_cols];
    let mut out_close = vec![f32::NAN; n_out * n_cols];
    let mut out_vol = vec![0.0_f32; n_out * n_cols];

    for col in 0..n_cols {
        for out_row in 0..n_out {
            let group_start = out_row * factor;
            let group_end = (group_start + factor).min(n_5m);

            let mut first_open = f32::NAN;
            let mut max_high = f32::NEG_INFINITY;
            let mut min_low = f32::INFINITY;
            let mut last_close = f32::NAN;
            let mut vol_sum = 0.0_f32;
            let mut any_valid = false;

            for in_row in group_start..group_end {
                let o = open_5m.get(in_row, col);
                let h = high_5m.get(in_row, col);
                let l = low_5m.get(in_row, col);
                let c = close_5m.get(in_row, col);
                let v = volume_5m.get(in_row, col);

                // Skip bars where all OHLC are NaN (e.g. no trading)
                if o.is_nan() && h.is_nan() && c.is_nan() {
                    continue;
                }
                any_valid = true;

                if first_open.is_nan() && !o.is_nan() {
                    first_open = o;
                }
                if !h.is_nan() && h > max_high {
                    max_high = h;
                }
                if !l.is_nan() && l < min_low {
                    min_low = l;
                }
                if !c.is_nan() {
                    last_close = c;
                }
                if !v.is_nan() {
                    vol_sum += v;
                }
            }

            if any_valid {
                let idx = out_row * n_cols + col;
                out_open[idx] = first_open;
                out_high[idx] = if max_high.is_finite() { max_high } else { f32::NAN };
                out_low[idx] = if min_low.is_finite() { min_low } else { f32::NAN };
                out_close[idx] = last_close;
                out_vol[idx] = vol_sum;
            }
        }
    }

    [
        WideMatrix::new(out_open, n_out, n_cols),
        WideMatrix::new(out_high, n_out, n_cols),
        WideMatrix::new(out_low, n_out, n_cols),
        WideMatrix::new(out_close, n_out, n_cols),
        WideMatrix::new(out_vol, n_out, n_cols),
    ]
}

/// Resample timestamps: take every `factor`-th timestamp (start of each group).
pub fn resample_timestamps(timestamps: &[i64], factor: usize) -> Vec<i64> {
    timestamps.iter().step_by(factor).copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_matrix(data: Vec<f32>, n_rows: usize, n_cols: usize) -> WideMatrix {
        WideMatrix::new(data, n_rows, n_cols)
    }

    #[test]
    fn resample_6_to_1_aggregates_correctly() {
        // 6 bars × 1 ticker: bars at prices 1..6
        let n = 6;
        let open_data: Vec<f32> = (1..=n).map(|i| i as f32).collect();
        let high_data: Vec<f32> = open_data.iter().map(|&x| x + 0.5).collect();
        let low_data: Vec<f32> = open_data.iter().map(|&x| x - 0.5).collect();
        let close_data = open_data.clone();
        let vol_data = vec![100.0_f32; n];

        let open_m = make_matrix(open_data.clone(), n, 1);
        let high_m = make_matrix(high_data, n, 1);
        let low_m = make_matrix(low_data, n, 1);
        let close_m = make_matrix(close_data, n, 1);
        let vol_m = make_matrix(vol_data, n, 1);

        let [o, h, l, c, v] = resample_ohlcv(&open_m, &high_m, &low_m, &close_m, &vol_m, 6);

        assert_eq!(o.n_rows(), 1);
        assert!((o.get(0, 0) - 1.0).abs() < 1e-5, "open should be first: {}", o.get(0, 0));
        assert!((h.get(0, 0) - 6.5).abs() < 1e-5, "high should be max: {}", h.get(0, 0));
        assert!((l.get(0, 0) - 0.5).abs() < 1e-5, "low should be min: {}", l.get(0, 0));
        assert!((c.get(0, 0) - 6.0).abs() < 1e-5, "close should be last: {}", c.get(0, 0));
        assert!((v.get(0, 0) - 600.0).abs() < 1e-5, "vol should be sum: {}", v.get(0, 0));
    }

    #[test]
    fn resample_partial_group_at_end() {
        // 7 bars with factor=6: should produce 2 output rows
        let n = 7;
        let data: Vec<f32> = (1..=n).map(|i| i as f32).collect();
        let m = make_matrix(data.clone(), n, 1);
        let vol = make_matrix(vec![1.0; n], n, 1);

        let [o, _, _, c, _] = resample_ohlcv(&m, &m, &m, &m, &vol, 6);
        assert_eq!(o.n_rows(), 2, "should produce 2 output rows");
        assert!((c.get(1, 0) - 7.0).abs() < 1e-5, "last close should be 7: {}", c.get(1, 0));
    }
}
