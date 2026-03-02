//! Pattern-driven breakout strategy using fuzzy bull flag scoring.
//!
//! Extends `SignalBreakout` with a parallel fuzzy pattern scoring pass across
//! four timeframes (daily, 1h, 30m, 5m). Entry score is the blend:
//!
//!   `alpha * signal_score + (1 - alpha) * pattern_score`
//!
//! where `alpha = params.pattern_params.pattern_alpha`.
//!
//! Falls back cleanly to pure signal scoring when intraday data is unavailable.

use std::ops::Range;

use engine_data::{DataStore, Indicator, WideMask, WideMatrix};
use engine_patterns::compute_pattern_score;
use engine_signals::arena::ThreadArena;
use engine_signals::pipeline::{self, simple_atr, CharacterizationState, run_pipeline, run_pattern_pipeline};
use engine_types::{Direction, PatternParams, Params, SignalSet};
use crate::execution::{ExitRule, FillMode};

use super::{Setup, atr_stop, simple_price_vol_filter};

pub struct PatternBreakout;

impl Setup for PatternBreakout {
    fn name(&self) -> &'static str {
        "pattern_breakout"
    }

    fn direction(&self) -> Direction {
        Direction::Long
    }

    fn fill_mode(&self) -> FillMode {
        FillMode::NextDayOpen
    }

    fn exit_rules(&self, params: &Params) -> Vec<ExitRule> {
        vec![
            ExitRule::ProfitTarget {
                min_bars: params.strategy.profit_target_min_bars as usize,
            },
            ExitRule::SmaCross { sma: Indicator::Sma10 },
            ExitRule::StopLoss,
        ]
    }

    fn filter(&self, store: &DataStore, params: &Params, range: Range<usize>) -> WideMask {
        simple_price_vol_filter(store, params, range)
    }

    fn signals(&self, store: &DataStore, universe: &WideMask, params: &Params) -> SignalSet {
        pattern_breakout_signals(store, universe, params)
    }
}

// ---------------------------------------------------------------------------
// Signal generation
// ---------------------------------------------------------------------------

fn pattern_breakout_signals(
    store: &DataStore,
    universe: &WideMask,
    params: &Params,
) -> SignalSet {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;
    let mut entries = WideMask::new_false(nr, nc);
    let mut exits = WideMask::new_false(nr, nc);
    let mut stops = vec![f32::NAN; nr * nc];

    let sp = params.signal_params.as_ref().cloned().unwrap_or_default();
    let pp: PatternParams = params.pattern_params.as_ref().cloned().unwrap_or_default();

    let close_m = store.close();
    let high_m = store.high();
    let low_m = store.low();
    let atr_m = store.get(Indicator::Atr14);
    let vol_m = store.get(Indicator::Volume);

    for col in 0..nc {
        let has_any = (0..nr).any(|r| universe.get(r, col));
        if !has_any {
            continue;
        }

        let mut arena = ThreadArena::new(&sp);
        let mut l0_state = CharacterizationState::default();

        // Extract full daily columns
        let close_col: Vec<f32> = (0..nr).map(|r| close_m.get(r, col)).collect();
        let high_col: Vec<f32> = (0..nr).map(|r| high_m.get(r, col)).collect();
        let low_col: Vec<f32> = (0..nr).map(|r| low_m.get(r, col)).collect();
        let vol_col: Vec<f32> = (0..nr).map(|r| vol_m.get(r, col)).collect();

        let returns_col: Vec<f32> = (1..nr)
            .map(|r| {
                let prev = close_col[r - 1];
                let cur = close_col[r];
                if prev > 0.0 && !prev.is_nan() && !cur.is_nan() {
                    (cur / prev).ln()
                } else {
                    0.0
                }
            })
            .collect();

        for row in 0..nr {
            if !universe.get(row, col) {
                continue;
            }
            let close = close_col[row];
            if close.is_nan() {
                continue;
            }

            let w_start = row.saturating_sub(sp.window_len.saturating_sub(1));
            let close_window = &close_col[w_start..=row];
            let vol_window = &vol_col[w_start..=row];

            if close_window.len() < params.strategy.min_signal_data as usize {
                continue;
            }

            // L0 characterization
            if row.saturating_sub(l0_state.last_updated) >= sp.l0_recompute_interval {
                let ret_end = row.min(returns_col.len());
                let ret_start = ret_end.saturating_sub(sp.hurst_window);
                let ret_slice = &returns_col[ret_start..ret_end];
                l0_state = pipeline::characterize(
                    close_window,
                    ret_slice,
                    None,
                    row,
                    &sp,
                    &l0_state,
                );
            }

            // Signal pipeline score
            let sig_out = run_pipeline(
                close_window,
                vol_window,
                &[],
                &[],
                &mut arena,
                &sp,
                &l0_state,
            );

            // Pattern score across timeframes
            let daily_atr = atr_m.get(row, col);
            let pattern_scores = compute_pattern_scores_for_bar(
                store,
                col,
                row,
                &close_col,
                &high_col,
                &low_col,
                &vol_col,
                daily_atr,
                &pp,
            );

            let pattern_composite = compute_pattern_score(
                &pattern_scores,
                &pp.pattern_scorer_weights,
                pp.pattern_scorer_bias,
            );

            let blended = pp.pattern_alpha * sig_out.score
                + (1.0 - pp.pattern_alpha) * pattern_composite;

            if blended > sp.candidate_threshold {
                let i = row * nc + col;
                entries.data[i] = true;
                stops[i] = atr_stop(
                    close,
                    low_m.get(row, col),
                    atr_m.get(row, col),
                    params.strategy.stop_fallback_pct,
                );
            }

            if sig_out.bocpd_exit {
                exits.data[row * nc + col] = true;
            }
        }
    }

    SignalSet {
        entries,
        exits,
        stop_prices: WideMatrix::new(stops, nr, nc),
    }
}

// ---------------------------------------------------------------------------
// Helper: extract intraday slices and run pattern pipeline for one bar
// ---------------------------------------------------------------------------

/// Compute pattern scores for one (col, row) position across 4 timeframes.
///
/// Daily: uses the prebuilt `close_col/high_col/low_col/vol_col` up to `row`.
/// 1h/30m/5m: extracted from `store.intraday` when available.
fn compute_pattern_scores_for_bar(
    store: &DataStore,
    col: usize,
    row: usize,
    daily_close: &[f32],
    daily_high: &[f32],
    daily_low: &[f32],
    daily_vol: &[f32],
    daily_atr: f32,
    pp: &PatternParams,
) -> [f32; 4] {
    // Daily slice up to and including current row
    let dc = &daily_close[..=row];
    let dh = &daily_high[..=row];
    let dl = &daily_low[..=row];
    let dv = &daily_vol[..=row];

    // Intraday slices: extract up to and including the last 5m bar for this day
    let (h1c, h1h, h1l, h1v, h1_atr) = extract_intraday_tf(store, col, row, 1);
    let (m30c, m30h, m30l, m30v, m30_atr) = extract_intraday_tf(store, col, row, 0);
    let (m5c, m5h, m5l, m5v, m5_atr) = extract_5m_tf(store, col, row);

    run_pattern_pipeline(
        dc, dh, dl, dv, daily_atr,
        &h1c, &h1h, &h1l, &h1v, h1_atr,
        &m30c, &m30h, &m30l, &m30v, m30_atr,
        &m5c, &m5h, &m5l, &m5v, m5_atr,
        pp,
    )
}

/// Extract intraday OHLCV column up to the current daily row.
///
/// `tf_idx`: 0 = 30m matrices, 1 = 1h matrices (indices into IntradayData).
/// Returns (close, high, low, volume, atr) — atr is a simple close-range estimate.
fn extract_intraday_tf(
    store: &DataStore,
    col: usize,
    daily_row: usize,
    tf_idx: usize, // 0=30m, 1=1h
) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, f32) {
    let Some(intraday) = &store.intraday else {
        return (vec![], vec![], vec![], vec![], f32::NAN);
    };

    // bars_per_day = trading_hours * bars_per_hour; 30m = ÷2, 1h = ÷1
    let bars_factor = if tf_idx == 0 { 2.0 } else { 1.0 }; // relative to 1h
    let n_rows_per_day = (store.axes.trading_hours as f32 * bars_factor).round() as usize;
    let n_rows_per_day = n_rows_per_day.max(1);

    let matrices = if tf_idx == 0 {
        &intraday.matrices_30m
    } else {
        &intraday.matrices_1h
    };

    if matrices.is_empty() {
        return (vec![], vec![], vec![], vec![], f32::NAN);
    }

    // Approximate row range: daily_row * bars_per_day (may overshoot, WideMatrix clamps)
    let end_row = ((daily_row + 1) * n_rows_per_day).min(matrices[0].n_rows());
    if end_row == 0 {
        return (vec![], vec![], vec![], vec![], f32::NAN);
    }

    let nc = matrices[0].n_cols();
    if col >= nc {
        return (vec![], vec![], vec![], vec![], f32::NAN);
    }

    let close: Vec<f32> = (0..end_row).map(|r| matrices[3].get(r, col)).collect(); // close=index 3
    let high: Vec<f32> = (0..end_row).map(|r| matrices[1].get(r, col)).collect();
    let low: Vec<f32> = (0..end_row).map(|r| matrices[2].get(r, col)).collect();
    let vol: Vec<f32> = (0..end_row).map(|r| matrices[4].get(r, col)).collect();

    let atr = simple_atr(&close, 14);
    (close, high, low, vol, atr)
}

/// Extract 5m OHLCV column up to the current daily row using `day_mapping`.
fn extract_5m_tf(
    store: &DataStore,
    col: usize,
    daily_row: usize,
) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, f32) {
    let Some(intraday) = &store.intraday else {
        return (vec![], vec![], vec![], vec![], f32::NAN);
    };

    if intraday.matrices.is_empty() || col >= intraday.matrices[0].n_cols() {
        return (vec![], vec![], vec![], vec![], f32::NAN);
    }

    let end_5m = if daily_row < intraday.day_mapping.len() {
        intraday.day_mapping[daily_row].1
    } else {
        intraday.matrices[0].n_rows()
    };

    if end_5m == 0 {
        return (vec![], vec![], vec![], vec![], f32::NAN);
    }

    let close: Vec<f32> = (0..end_5m).map(|r| intraday.matrices[3].get(r, col)).collect();
    let high: Vec<f32> = (0..end_5m).map(|r| intraday.matrices[1].get(r, col)).collect();
    let low: Vec<f32> = (0..end_5m).map(|r| intraday.matrices[2].get(r, col)).collect();
    let vol: Vec<f32> = (0..end_5m).map(|r| intraday.matrices[4].get(r, col)).collect();

    let atr = simple_atr(&close, 14);
    (close, high, low, vol, atr)
}
