//! Signal-driven breakout strategy.
//!
//! Implements the `Setup` trait using the 4-layer signal processing pipeline
//! (see `signals/pipeline.rs`) instead of the traditional VCP/flag pattern
//! detectors. Universe screening reuses the same price/volume/ETF filters as
//! the classic breakout; entry decisions come from the composite signal score
//! and exit decisions are driven by BOCPD regime-death and Kalman trailing
//! stops.
//!
//! While the pipeline algorithms are stubs (`run_pipeline` returns defaults),
//! this strategy integrates cleanly and will produce live signals once the
//! per-algorithm `compute_*` functions are implemented.

use std::ops::Range;

use engine_data::{DataStore, Indicator, WideMask, WideMatrix};
use engine_signals::arena::ThreadArena;
use engine_signals::pipeline::{self, CharacterizationState, run_pipeline};
use crate::execution::{ExitRule, FillMode};
use engine_types::{Direction, Params, SignalSet};

use super::{Setup, atr_stop, simple_price_vol_filter};

pub struct SignalBreakout;

impl Setup for SignalBreakout {
    fn name(&self) -> &'static str {
        "signal_breakout"
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
        signal_breakout_signals(store, universe, params)
    }
}

// ---------------------------------------------------------------------------
// Signal generation
// ---------------------------------------------------------------------------

/// Run the signal pipeline per ticker/bar and produce entry/exit/stop signals.
///
/// For each column (ticker) that passes the universe filter, extracts a
/// trailing close window, feeds it through `run_pipeline()`, and maps the
/// composite output to entry/exit/stop masks. Processes tickers sequentially
/// because `signals()` is called from a single-threaded context (rayon
/// parallelism happens at the simulation level).
fn signal_breakout_signals(store: &DataStore, universe: &WideMask, params: &Params) -> SignalSet {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;
    let mut entries = WideMask::new_false(nr, nc);
    let mut exits = WideMask::new_false(nr, nc);
    let mut stops = vec![f32::NAN; nr * nc];

    let sp = params
        .signal_params
        .as_ref()
        .cloned()
        .unwrap_or_default();

    let close_m = store.close();
    let atr_m = store.get(Indicator::Atr14);
    let low_m = store.low();
    let vol_m = store.get(Indicator::Volume);

    for col in 0..nc {
        // Skip tickers with no universe rows at all (fast path)
        let has_any = (0..nr).any(|r| universe.get(r, col));
        if !has_any {
            continue;
        }

        let mut arena = ThreadArena::new(&sp);
        let mut l0_state = CharacterizationState::default();

        // Extract full close and volume columns for trailing window slicing
        let close_col: Vec<f32> = (0..nr).map(|r| close_m.get(r, col)).collect();
        let vol_col: Vec<f32> = (0..nr).map(|r| vol_m.get(r, col)).collect();

        // Compute log-returns for L0 characterization
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

            // Extract trailing window ending at current bar
            let w_start = row.saturating_sub(sp.window_len.saturating_sub(1));
            let close_window = &close_col[w_start..=row];
            let vol_window = &vol_col[w_start..=row];

            // Need minimum data for meaningful signal computation
            if close_window.len() < params.strategy.min_signal_data as usize {
                continue;
            }

            // L0: Recompute characterization when interval elapsed
            if row.saturating_sub(l0_state.last_updated) >= sp.l0_recompute_interval {
                let ret_end = row.min(returns_col.len());
                let ret_start = ret_end.saturating_sub(sp.hurst_window);
                let ret_slice = &returns_col[ret_start..ret_end];
                l0_state = pipeline::characterize(
                    close_window,
                    ret_slice,
                    None, // multi-asset returns (RMT) not available per-ticker
                    row,
                    &sp,
                    &l0_state,
                );
            }

            let output = run_pipeline(
                close_window,
                vol_window,
                &[], // high_window (reserved)
                &[], // low_window (reserved)
                &mut arena,
                &sp,
                &l0_state,
            );

            if output.is_candidate {
                let i = row * nc + col;
                entries.data[i] = true;
                stops[i] = atr_stop(
                    close,
                    low_m.get(row, col),
                    atr_m.get(row, col),
                    params.strategy.stop_fallback_pct,
                );
            }

            // BOCPD exit signal (from Layer 3 reusing L1 state)
            if output.bocpd_exit {
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
