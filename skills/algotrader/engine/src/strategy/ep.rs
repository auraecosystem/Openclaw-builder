//! Episodic Pivot (EP) strategy: gap-up catalyst plays with prior neglect.
//!
//! Identifies stocks that gap up sharply on high volume after a period of
//! sideways/neglected price action. Entry is shifted to the day after the gap
//! to avoid lookahead bias. Uses SameDayOpen fill mode because the signal
//! already points at the entry day.

use std::ops::Range;

use engine_data::{DataStore, Indicator, WideMask, WideMatrix};
use crate::execution::{ExitRule, FillMode};
use engine_types::{Direction, Params, SignalSet};

use super::Setup;

// ---------------------------------------------------------------------------
// Universe filter
// ---------------------------------------------------------------------------

/// Build the EP universe mask. Ported from the former `build_ep_universe`.
///
/// Filters (no regime filter -- EPs work across market conditions per
/// Qullamaggie rules):
/// - Minimum price and volume
/// - Minimum average dollar volume (ADV)
/// - Gap-up >= 10% from prior close
/// - Volume spike >= 2x the 20-day volume SMA
/// - Gap-day dollar volume >= $100M
/// - Prior neglect: 6-month return < 30%
fn ep_filter(store: &DataStore, params: &Params, range: Range<usize>) -> WideMask {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;

    let min_gap_pct: f32 = 0.10;
    let min_vol_ratio: f32 = 2.0;
    let min_gap_day_dollar_vol: f32 = 100_000_000.0;
    let max_prior_6m_return: f32 = 0.30;

    let close_m = store.close();
    let open_m = store.open();
    let vol_m = store.volume();
    let vol_sma = store.get(Indicator::VolSma20);
    let ret_126 = store.get(Indicator::Ret126);

    let mut mask = WideMask::new_false(nr, nc);

    // Start from row 1 (need prior close for gap calculation)
    let effective_start = range.start.max(1);
    let effective_end = range.end.min(nr);

    for row in effective_start..effective_end {
        for col in 0..nc {
            if store.axes.etf_cols[col] {
                continue;
            }
            let close = close_m.get(row, col);
            if close.is_nan() || close <= params.min_price {
                continue;
            }
            let vsma = vol_sma.get(row, col);
            if vsma.is_nan() || vsma <= params.min_vol {
                continue;
            }
            if vsma * close < params.min_adv {
                continue;
            }

            // Gap: open / prev_close - 1 >= 10%
            let prev_close = close_m.get(row - 1, col);
            let open_val = open_m.get(row, col);
            if prev_close.is_nan() || prev_close <= 0.0 {
                continue;
            }
            let gap = open_val / prev_close - 1.0;
            if gap < min_gap_pct {
                continue;
            }

            // Volume spike
            let vol = vol_m.get(row, col);
            if vol < min_vol_ratio * vsma {
                continue;
            }

            // Gap-day dollar volume >= $100M
            if vol * close < min_gap_day_dollar_vol {
                continue;
            }

            // Prior neglect: stock should NOT have rallied in prior 6 months
            let ret_6m = ret_126.get(row, col);
            if !ret_6m.is_nan() && ret_6m >= max_prior_6m_return {
                continue;
            }

            mask.set(row, col, true);
        }
    }
    mask
}

// ---------------------------------------------------------------------------
// Signal generation
// ---------------------------------------------------------------------------

/// Generate EP entry/exit/stop signals. Ported from the former `ep_signals`.
///
/// Entry is shifted to the day AFTER the gap (avoids lookahead). Stop is set
/// to the gap day's low. Exit when close < SMA10.
fn ep_signals(store: &DataStore, universe: &WideMask, _params: &Params) -> SignalSet {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;
    let mut entries = WideMask::new_false(nr, nc);
    let mut exits = WideMask::new_false(nr, nc);
    let mut stops = vec![f32::NAN; nr * nc];

    let close_m = store.close();
    let sma10 = store.get(Indicator::Sma10);
    let low_m = store.low();

    for row in 0..nr {
        for col in 0..nc {
            let i = row * nc + col;
            let close = close_m.get(row, col);
            let sma = sma10.get(row, col);

            // SMA trail exit applies everywhere
            if !close.is_nan() && !sma.is_nan() && close < sma {
                exits.data[i] = true;
            }

            if !universe.get(row, col) {
                continue;
            }

            // Universe already checks gap + volume spike + liquidity.
            // Mark the NEXT day as entry (avoid lookahead on the gap day itself).
            if row + 1 < nr {
                let next_i = (row + 1) * nc + col;
                entries.data[next_i] = true;
                // Stop: gap day's low
                stops[next_i] = low_m.get(row, col);
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
// Setup implementation
// ---------------------------------------------------------------------------

/// Episodic Pivot setup. Buys the day after a high-volume catalyst gap-up on
/// a previously neglected stock.
pub struct EpisodicPivot;

impl Setup for EpisodicPivot {
    fn name(&self) -> &'static str {
        "ep"
    }

    fn direction(&self) -> Direction {
        Direction::Long
    }

    /// EP uses same-day open because the signal is already shifted to entry day.
    fn fill_mode(&self) -> FillMode {
        FillMode::SameDayOpen
    }

    fn exit_rules(&self, _params: &Params) -> Vec<ExitRule> {
        vec![
            ExitRule::SmaCross {
                sma: Indicator::Sma10,
            },
            ExitRule::StopLoss,
        ]
    }

    fn filter(&self, store: &DataStore, params: &Params, range: Range<usize>) -> WideMask {
        ep_filter(store, params, range)
    }

    fn signals(&self, store: &DataStore, universe: &WideMask, params: &Params) -> SignalSet {
        ep_signals(store, universe, params)
    }
}
