//! Breakout strategy: VCP/flag consolidation breakout on volume expansion.
//!
//! Two sub-strategies share identical filter and signal logic but differ in
//! exit management:
//! - **BreakoutQuick**: takes profit after min_bars if profitable, trails SMA10.
//! - **BreakoutRunner**: upgrades stop to breakeven after N bars, trails SMA10.
//!
//! The equity split between quick and runner is controlled by `Params::split_frac`;
//! each struct returns a fixed 0.5 default. The caller (lib.rs) applies the
//! actual split_frac from params at simulation time.

use std::ops::Range;

use crate::data::{DataStore, Indicator, WideMask, WideMatrix};
use crate::execution::{ExitRule, FillMode};
use crate::types::{Direction, Params, SignalSet};

use super::Setup;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Compute regime filter: true = risk-on (10-day EMA > 20-day EMA of benchmark).
/// Uses the benchmark column (SPY/QQQ for stocks, BTCUSDT for crypto).
/// When no benchmark is available, every row passes.
fn compute_regime(store: &DataStore) -> Vec<bool> {
    let nr = store.axes.n_rows;
    let mut regime = vec![true; nr];

    let Some(bench_col) = store.axes.spy_col else {
        return regime;
    };

    // Standard EMA: alpha = 2 / (period + 1)
    let alpha10: f64 = 2.0 / 11.0;
    let alpha20: f64 = 2.0 / 21.0;
    let mut ema10: f64 = f64::NAN;
    let mut ema20: f64 = f64::NAN;
    let mut count = 0usize;

    for (row, regime_val) in regime.iter_mut().enumerate() {
        let val = store.close().get(row, bench_col) as f64;
        if val.is_nan() {
            continue;
        }
        count += 1;

        if count == 1 {
            ema10 = val;
            ema20 = val;
        } else {
            ema10 = alpha10 * val + (1.0 - alpha10) * ema10;
            ema20 = alpha20 * val + (1.0 - alpha20) * ema20;
        }

        // EMA needs at least 20 bars to be meaningful
        *regime_val = if count >= 20 { ema10 >= ema20 } else { true };
    }
    regime
}

/// Build the breakout universe mask. Ported from the former `build_breakout_universe`.
///
/// Filters applied (fused single-pass):
/// - Regime (optional): benchmark EMA10 > EMA20
/// - Minimum price and volume
/// - Relative strength: top N% across 1m/3m/6m timeframes
/// - Near 52-week high
/// - Prior 3-month move
/// - Extension cap: price not > 1x ATR above consolidation high
/// - ADR% floor
/// - Consolidation duration (min consecutive days within range)
fn breakout_filter(store: &DataStore, params: &Params, range: Range<usize>) -> WideMask {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;
    let rs_threshold = 1.0 - params.rs_pct;
    let regime = if params.regime {
        compute_regime(store)
    } else {
        vec![true; nr]
    };

    let close_m = store.close();
    let vol_sma = store.get(Indicator::VolSma20);
    let rs_1m = store.get(Indicator::RsPctrank1m);
    let rs_3m = store.get(Indicator::RsPctrank3m);
    let rs_6m = store.get(Indicator::RsPctrank6m);
    let dist_52w = store.get(Indicator::Dist52w);
    let ret_63 = store.get(Indicator::Ret63);
    let consol_high = store.get(Indicator::ConsolHigh);
    let atr_14 = store.get(Indicator::Atr14);
    let vcp_last = store.get(Indicator::VcpLastContractionPct);

    let mut mask = WideMask::new_false(nr, nc);

    for row in range {
        if !regime[row] {
            continue;
        }
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
            // RS filter: top N% across all three timeframes
            if rs_1m.get(row, col) < rs_threshold
                || rs_3m.get(row, col) < rs_threshold
                || rs_6m.get(row, col) < rs_threshold
            {
                continue;
            }
            // Near 52-week high
            if dist_52w.get(row, col) > params.max_dist_52w {
                continue;
            }
            // Prior 3M return
            if ret_63.get(row, col) < params.min_prior_move {
                continue;
            }
            // Extension filter: price not > 1x ATR above consolidation high
            let ch = consol_high.get(row, col);
            let atr = atr_14.get(row, col);
            if !ch.is_nan() && !atr.is_nan() && atr > 0.0 && (close - ch) > atr {
                continue;
            }
            // ADR% floor
            let atr2 = atr_14.get(row, col);
            if !atr2.is_nan() && close > 0.0 && (atr2 / close) < params.min_adr_pct {
                continue;
            }
            // Consolidation duration
            if params.min_consol_days > 0 {
                let needed = params.min_consol_days as usize;
                let start = row.saturating_sub(needed - 1);
                let count = (start..=row)
                    .rev()
                    .take_while(|&r| {
                        let br = vcp_last.get(r, col);
                        !br.is_nan() && br < params.max_range_pct
                    })
                    .count();
                if count < needed {
                    continue;
                }
            }

            mask.set(row, col, true);
        }
    }
    mask
}

/// Generate breakout entry/exit/stop signals. Ported from the former `breakout_signals`.
///
/// Entry requires: universe pass AND (VCP or flag pattern) AND close > consolidation
/// high AND volume spike AND ADV filter.
/// Exit: close < SMA10 (trail).
/// Stop: low-of-day, capped at 1x ATR below close.
fn breakout_signals(store: &DataStore, universe: &WideMask, params: &Params) -> SignalSet {
    let nr = store.axes.n_rows;
    let nc = store.axes.n_cols;
    let mut entries = WideMask::new_false(nr, nc);
    let mut exits = WideMask::new_false(nr, nc);
    let mut stops = vec![f32::NAN; nr * nc];

    let close_m = store.close();
    let sma10 = store.get(Indicator::Sma10);
    let vcp_nc = store.get(Indicator::VcpNumContractions);
    let vcp_last = store.get(Indicator::VcpLastContractionPct);
    let vcp_tight = store.get(Indicator::VcpTighteningRatio);
    let vcp_vol = store.get(Indicator::VcpVolTrend);
    let flag_pole = store.get(Indicator::FlagPolePct);
    let flag_ret = store.get(Indicator::FlagRetracePct);
    let flag_d = store.get(Indicator::FlagDays);
    let flag_vr = store.get(Indicator::FlagVolRatio);
    let consol_h_m = store.get(Indicator::ConsolHigh);
    let vol_m = store.volume();
    let vol_sma = store.get(Indicator::VolSma20);
    let low_m = store.low();
    let atr_m = store.get(Indicator::Atr14);

    for row in 0..nr {
        for col in 0..nc {
            let i = row * nc + col;
            let close = close_m.get(row, col);
            let sma = sma10.get(row, col);

            // SMA trail exit applies everywhere (not just universe)
            if !close.is_nan() && !sma.is_nan() && close < sma {
                exits.data[i] = true;
            }

            if !universe.get(row, col) {
                continue;
            }

            // VCP: >= 2 progressive contractions, last contraction tight, volume drying
            let is_vcp = vcp_nc.get(row, col) >= 2.0
                && vcp_last.get(row, col) < params.max_range_pct
                && vcp_tight.get(row, col) < 1.0
                && vcp_vol.get(row, col) < 1.0;

            // Flag: pole >= 20%, retrace < 50%, 5-25 day flag, volume drying
            let fp = flag_pole.get(row, col);
            let fr = flag_ret.get(row, col);
            let fd = flag_d.get(row, col);
            let fv = flag_vr.get(row, col);
            let is_flag =
                fp >= 0.20 && fr > 0.0 && fr < 0.50 && (5.0..=25.0).contains(&fd) && fv < 1.0;

            if !is_vcp && !is_flag {
                continue;
            }

            // Breakout: close > consolidation high
            let consol_h = consol_h_m.get(row, col);
            if consol_h.is_nan() || close <= consol_h {
                continue;
            }

            // Volume spike
            let vol = vol_m.get(row, col);
            let vsma = vol_sma.get(row, col);
            if vol <= params.vol_ratio * vsma {
                continue;
            }

            // ADV filter
            if vsma * close < params.min_adv {
                continue;
            }

            entries.data[i] = true;

            // Stop: low-of-day, capped at 1x ATR below close
            let low = low_m.get(row, col);
            let atr = atr_m.get(row, col);
            let stop = if !atr.is_nan() && close > 0.0 && (close - low) > atr {
                close - atr
            } else {
                low
            };
            stops[i] = stop;
        }
    }

    SignalSet {
        entries,
        exits,
        stop_prices: WideMatrix::new(stops, nr, nc),
    }
}

// ---------------------------------------------------------------------------
// BreakoutQuick
// ---------------------------------------------------------------------------

/// Quick-exit half of the breakout strategy. Takes profit after min_bars if the
/// position is profitable, otherwise trails SMA10 with a hard stop loss.
pub struct BreakoutQuick;

impl Setup for BreakoutQuick {
    fn name(&self) -> &'static str {
        "breakout_quick"
    }

    fn direction(&self) -> Direction {
        Direction::Long
    }

    fn equity_fraction(&self) -> f32 {
        0.5
    }

    fn fill_mode(&self) -> FillMode {
        FillMode::NextDayOpen
    }

    fn exit_rules(&self) -> Vec<ExitRule> {
        vec![
            ExitRule::ProfitTarget { min_bars: 5 },
            ExitRule::SmaCross {
                sma: Indicator::Sma10,
            },
            ExitRule::StopLoss,
        ]
    }

    fn filter(&self, store: &DataStore, params: &Params, range: Range<usize>) -> WideMask {
        breakout_filter(store, params, range)
    }

    fn signals(&self, store: &DataStore, universe: &WideMask, params: &Params) -> SignalSet {
        breakout_signals(store, universe, params)
    }
}

// ---------------------------------------------------------------------------
// BreakoutRunner
// ---------------------------------------------------------------------------

/// Runner half of the breakout strategy. Upgrades stop to breakeven after N bars
/// if profitable, otherwise trails SMA10 with a hard stop loss. Designed to
/// capture extended moves.
pub struct BreakoutRunner;

impl Setup for BreakoutRunner {
    fn name(&self) -> &'static str {
        "breakout_runner"
    }

    fn direction(&self) -> Direction {
        Direction::Long
    }

    fn equity_fraction(&self) -> f32 {
        0.5
    }

    fn fill_mode(&self) -> FillMode {
        FillMode::NextDayOpen
    }

    fn exit_rules(&self) -> Vec<ExitRule> {
        vec![
            ExitRule::BreakevenUpgrade { after_bars: 5 },
            ExitRule::SmaCross {
                sma: Indicator::Sma10,
            },
            ExitRule::StopLoss,
        ]
    }

    fn filter(&self, store: &DataStore, params: &Params, range: Range<usize>) -> WideMask {
        breakout_filter(store, params, range)
    }

    fn signals(&self, store: &DataStore, universe: &WideMask, params: &Params) -> SignalSet {
        breakout_signals(store, universe, params)
    }
}
