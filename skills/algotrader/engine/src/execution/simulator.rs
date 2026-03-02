//! Generic simulation loop that replaces the three copy-pasted loops in
//! the original simulate.rs (breakout_quick, breakout_runner, ticker_single).
//!
//! One parameterized function handles all strategy variants by accepting:
//! - `exit_rules` -- which conditions close a position
//! - `sizer` -- how to compute position size and slippage
//! - `fill_mode` -- how to resolve the entry fill price
//! - `direction` -- Long or Short (affects PnL sign and slippage direction)
//! - `equity_fraction` -- portion of init_cash allocated (1.0 for full, split_frac for quick half)

use rayon::prelude::*;

use engine_data::{DataStore, Indicator};
use engine_types::{Direction, ResolvedParams, SignalSet, Trade};

use super::exits::{ExitContext, ExitRule};
use super::fills::{check_5m_stop, resolve_fill, FillMode};
use super::position::PositionSizer;

/// Run the simulation across all tickers with entry signals, returning trades.
///
/// The outer loop finds active columns (tickers with at least one entry signal
/// in the date range), then parallelizes across tickers via rayon. Each ticker
/// runs a sequential bar-by-bar scan that checks exits when in a position and
/// attempts entries when flat.
#[allow(clippy::too_many_arguments)]
pub fn simulate(
    store: &DataStore,
    signals: &SignalSet,
    rp: &ResolvedParams,
    sizer: &PositionSizer,
    exit_rules: &[ExitRule],
    direction: Direction,
    equity_fraction: f32,
    fill_mode: FillMode,
    setup_name: &'static str,
) -> Vec<Trade> {
    let nc = store.axes.n_cols;

    // Pre-filter to columns with at least one entry signal in range
    let active_cols: Vec<usize> = (0..nc)
        .filter(|&col| (rp.start_row..rp.end_row).any(|row| signals.entries.get(row, col)))
        .collect();

    if active_cols.is_empty() {
        return Vec::new();
    }

    // Parallel across tickers, sequential within each ticker
    active_cols
        .par_iter()
        .flat_map(|&col| {
            simulate_ticker(
                store,
                signals,
                rp,
                sizer,
                exit_rules,
                direction,
                equity_fraction,
                fill_mode,
                setup_name,
                col,
            )
        })
        .collect()
}

/// Single-ticker sequential simulation loop.
#[allow(clippy::too_many_arguments)]
fn simulate_ticker(
    store: &DataStore,
    signals: &SignalSet,
    rp: &ResolvedParams,
    sizer: &PositionSizer,
    exit_rules: &[ExitRule],
    direction: Direction,
    equity_fraction: f32,
    fill_mode: FillMode,
    setup_name: &'static str,
    col: usize,
) -> Vec<Trade> {
    let params = &rp.params;
    let equity = params.init_cash * equity_fraction as f64;

    let has_intraday = params.bars_per_day > 1 && store.intraday.is_some();

    let mut trades = Vec::new();
    let mut in_position = false;
    let mut entry_row: usize = 0;
    let mut entry_price: f32 = 0.0;
    let mut active_stop: f32 = f32::NAN;
    let mut bars_since: usize = 0;
    let mut shares: f32 = 0.0;
    let mut initial_stop_price: f32 = f32::NAN;

    for row in rp.start_row..rp.end_row {
        let close = store.close().get(row, col);
        if close.is_nan() {
            continue;
        }

        // --- Exit check (when holding a position) ---
        if in_position {
            bars_since += 1;

            // Run stop updates first (e.g. breakeven upgrade) so the active
            // stop is current before exit checks fire.
            for rule in exit_rules {
                let ctx = ExitContext {
                    row,
                    col,
                    close,
                    entry_price,
                    bars_since_entry: bars_since,
                    active_stop,
                    direction,
                };
                if let Some(new_stop) = rule.update_stop(&ctx) {
                    active_stop = new_stop;
                }
            }

            // Check intraday 5m stop if available
            let intraday_stop = if has_intraday && !active_stop.is_nan() {
                check_5m_stop(store, row, col, active_stop, direction)
            } else {
                None
            };

            // If the intraday stop fired, that overrides the daily close for
            // the StopLoss rule. We short-circuit here since the position is
            // definitely closed.
            let mut exited = false;

            if let Some(intraday_exit) = intraday_stop {
                // Intraday stop hit -- exit at the 5m price
                let pnl = compute_pnl(direction, intraday_exit, entry_price, shares);
                trades.push(Trade {
                    ticker_col: col,
                    setup: setup_name,
                    entry_row,
                    exit_row: row,
                    entry_price,
                    exit_price: intraday_exit,
                    initial_stop: initial_stop_price,
                    shares,
                    pnl,
                    pnl_pct: pnl / (entry_price as f64 * shares as f64),
                    direction,
                });
                in_position = false;
                exited = true;
            }

            if !exited {
                // Pre-computed signal exits (e.g. BOCPD regime-death) close
                // at market price. Skip during the min_hold_bars window to
                // avoid conflating the entry breakout with a changepoint signal.
                if bars_since >= params.execution.min_hold_bars as usize
                    && signals.exits.get(row, col)
                {
                    let pnl = compute_pnl(direction, close, entry_price, shares);
                    trades.push(Trade {
                        ticker_col: col,
                        setup: setup_name,
                        entry_row,
                        exit_row: row,
                        entry_price,
                        exit_price: close,
                        initial_stop: initial_stop_price,
                        shares,
                        pnl,
                        pnl_pct: pnl / (entry_price as f64 * shares as f64),
                        direction,
                    });
                    in_position = false;
                    exited = true;
                }
            }

            if !exited {
                // Check each exit rule in priority order
                let ctx = ExitContext {
                    row,
                    col,
                    close,
                    entry_price,
                    bars_since_entry: bars_since,
                    active_stop,
                    direction,
                };

                for rule in exit_rules {
                    if rule.should_exit(&ctx, store) {
                        let pnl = compute_pnl(direction, close, entry_price, shares);
                        trades.push(Trade {
                            ticker_col: col,
                            setup: setup_name,
                            entry_row,
                            exit_row: row,
                            entry_price,
                            exit_price: close,
                            initial_stop: initial_stop_price,
                            shares,
                            pnl,
                            pnl_pct: pnl / (entry_price as f64 * shares as f64),
                            direction,
                        });
                        in_position = false;
                        break;
                    }
                }
            }
        }

        // --- Entry check (when flat) ---
        if !in_position && signals.entries.get(row, col) {
            let fill_result = resolve_fill(store, row, col, fill_mode, rp.end_row);
            let (fill_price, actual_entry_row) = match fill_result {
                Some(f) => f,
                None => continue,
            };

            let stop_price = signals.stop_prices.get(row, col);
            if stop_price.is_nan() {
                continue;
            }

            let price_risk = (fill_price - stop_price).abs();
            if price_risk <= 0.0 {
                continue;
            }

            let adv = store.get(Indicator::VolSma20).get(row, col);
            let (sized_shares, adjusted_fill) =
                sizer.compute(equity, fill_price, stop_price, adv, close, direction);

            entry_row = actual_entry_row;
            entry_price = adjusted_fill;
            active_stop = stop_price;
            initial_stop_price = stop_price;
            bars_since = 0;
            shares = sized_shares;
            in_position = true;
        }
    }

    trades
}

/// Compute PnL accounting for direction.
#[inline]
fn compute_pnl(direction: Direction, exit_price: f32, entry_price: f32, shares: f32) -> f64 {
    match direction {
        Direction::Long => (exit_price - entry_price) as f64 * shares as f64,
        Direction::Short => (entry_price - exit_price) as f64 * shares as f64,
    }
}
