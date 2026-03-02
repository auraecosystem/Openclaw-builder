//! Parabolic Short strategy: short overextended stocks on the first red day.
//!
//! Targets stocks that have made a parabolic run (large-cap +50% in 10 days,
//! small-cap +300%) and are showing the first reversal signal. No regime filter
//! because short setups profit from market weakness.

use std::ops::Range;

use engine_data::{DataStore, Indicator, WideMask, WideMatrix};
use crate::execution::{ExitRule, FillMode};
use engine_types::{Direction, Params, SignalSet};

use super::Setup;

// ---------------------------------------------------------------------------
// Universe filter
// ---------------------------------------------------------------------------

/// Build the parabolic short universe mask. Ported from `build_parabolic_universe`.
///
/// Filters (no regime filter -- short setups profit from downtrends):
/// - Minimum price and volume
/// - Minimum average dollar volume (ADV)
/// - Parabolic run: large-cap (>$50) needs +50% in 10d, small-cap needs +300%
/// - First red day: prior day had 3+ consecutive green days AND today close < open
fn parabolic_filter(store: &DataStore, params: &Params, range: Range<usize>) -> WideMask {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;

    let large_cap_price = params.strategy.large_cap_price;
    let large_cap_run = params.strategy.large_cap_run;
    let small_cap_run = params.strategy.small_cap_run;
    let min_green_days: f32 = 3.0;

    let close_m = store.close();
    let open_m = store.open();
    let vol_sma = store.get(Indicator::VolSma20);
    let pct_10d = store.get(Indicator::Pct10d);
    let consec_green = store.get(Indicator::ConsecGreen);

    let mut mask = WideMask::new_false(nr, nc);

    let effective_end = range.end.min(nr);

    for row in range.start..effective_end {
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

            // Parabolic run threshold depends on market cap proxy (price)
            let pct = pct_10d.get(row, col);
            let run_ok = if close > large_cap_price {
                pct >= large_cap_run
            } else {
                pct >= small_cap_run
            };
            if !run_ok {
                continue;
            }

            // First red day: today's close < open
            let open_val = open_m.get(row, col);
            if close >= open_val {
                continue;
            }

            // Prior day must have had 3+ consecutive green days
            if row == 0 {
                continue;
            }
            let prev_green = consec_green.get(row - 1, col);
            if prev_green < min_green_days {
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

/// Generate parabolic short entry/exit/stop signals. Ported from `parabolic_signals`.
///
/// Entry: universe pass (first red day after parabolic run).
/// Exit: close <= SMA10 or close <= SMA20 (mean reversion target for covering).
/// Stop: entry day's high (if price reclaims the high, the short thesis is wrong).
fn parabolic_signals(store: &DataStore, universe: &WideMask, _params: &Params) -> SignalSet {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;
    let mut entries = WideMask::new_false(nr, nc);
    let mut exits = WideMask::new_false(nr, nc);
    let mut stops = vec![f32::NAN; nr * nc];

    let close_m = store.close();
    let sma10 = store.get(Indicator::Sma10);
    let sma20 = store.get(Indicator::Sma20);
    let high_m = store.high();

    for row in 0..nr {
        for col in 0..nc {
            let i = row * nc + col;
            let close = close_m.get(row, col);
            let s10 = sma10.get(row, col);
            let s20 = sma20.get(row, col);

            // Cover when price reverts to either SMA (mean reversion target)
            if !close.is_nan()
                && ((!s10.is_nan() && close <= s10) || (!s20.is_nan() && close <= s20))
            {
                exits.data[i] = true;
            }

            if !universe.get(row, col) {
                continue;
            }

            // Universe already checks run + first red day + liquidity
            entries.data[i] = true;
            // Stop: entry day's high (if price reclaims, short is wrong)
            stops[i] = high_m.get(row, col);
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

/// Parabolic short setup. Shorts overextended stocks on the first red day
/// after a parabolic run, targeting mean reversion to the 10/20-day SMA.
pub struct ParabolicShort;

impl Setup for ParabolicShort {
    fn name(&self) -> &'static str {
        "parabolic"
    }

    fn direction(&self) -> Direction {
        Direction::Short
    }

    fn fill_mode(&self) -> FillMode {
        FillMode::NextDayOpen
    }

    fn exit_rules(&self, _params: &Params) -> Vec<ExitRule> {
        vec![
            ExitRule::SmaCover {
                sma1: Indicator::Sma10,
                sma2: Indicator::Sma20,
            },
            ExitRule::StopLoss,
        ]
    }

    fn filter(&self, store: &DataStore, params: &Params, range: Range<usize>) -> WideMask {
        parabolic_filter(store, params, range)
    }

    fn signals(&self, store: &DataStore, universe: &WideMask, params: &Params) -> SignalSet {
        parabolic_signals(store, universe, params)
    }
}
