//! Unit tests for each `ExitRule` variant's `should_exit()` and `update_stop()`.
//!
//! SmaCross and SmaCover tests require a minimal DataStore with indicator
//! matrices populated at specific (row, col) positions. The `make_test_store`
//! helper constructs one from a sparse list of (Indicator, values) pairs.

use std::collections::HashMap;
use strum::EnumCount;

use algotrader_engine::data::{Axes, DataStore, Indicator, WideMatrix};
use algotrader_engine::execution::{ExitContext, ExitRule};
use algotrader_engine::types::Direction;

// ---------------------------------------------------------------------------
// Helper: build a minimal DataStore for testing exit rules
// ---------------------------------------------------------------------------

/// Create a DataStore with `n_rows` x `n_cols` shape. All matrices default to
/// NaN. Only the (Indicator, data) pairs in `values` are overwritten.
fn make_test_store(n_rows: usize, n_cols: usize, values: &[(Indicator, Vec<f32>)]) -> DataStore {
    let mut daily: Vec<WideMatrix> = (0..Indicator::COUNT)
        .map(|_| WideMatrix::new(vec![f32::NAN; n_rows * n_cols], n_rows, n_cols))
        .collect();

    for (ind, data) in values {
        daily[*ind as usize] = WideMatrix::new(data.clone(), n_rows, n_cols);
    }

    let axes = Axes {
        dates: (0..n_rows as i32).collect(),
        tickers: (0..n_cols).map(|i| format!("T{}", i)).collect(),
        ticker_idx: HashMap::new(),
        spy_col: None,
        etf_cols: vec![false; n_cols],
        n_rows,
        n_cols,
        trading_hours: 6.5,
    };

    DataStore::new(axes, daily, None)
}

// ---------------------------------------------------------------------------
// StopLoss
// ---------------------------------------------------------------------------

#[test]
fn stop_loss_long_close_below_stop_exits() {
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 44.0,
        entry_price: 50.0,
        bars_since_entry: 3,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    assert!(
        ExitRule::StopLoss.should_exit(&ctx, &store),
        "long close 44 <= stop 45 should exit"
    );
}

#[test]
fn stop_loss_long_close_equals_stop_exits() {
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 45.0,
        entry_price: 50.0,
        bars_since_entry: 3,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    assert!(
        ExitRule::StopLoss.should_exit(&ctx, &store),
        "long close == stop should exit"
    );
}

#[test]
fn stop_loss_long_close_above_stop_no_exit() {
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 46.0,
        entry_price: 50.0,
        bars_since_entry: 3,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    assert!(
        !ExitRule::StopLoss.should_exit(&ctx, &store),
        "long close 46 > stop 45 should NOT exit"
    );
}

#[test]
fn stop_loss_short_close_above_stop_exits() {
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 56.0,
        entry_price: 50.0,
        bars_since_entry: 2,
        active_stop: 55.0,
        direction: Direction::Short,
    };
    assert!(
        ExitRule::StopLoss.should_exit(&ctx, &store),
        "short close 56 >= stop 55 should exit"
    );
}

#[test]
fn stop_loss_short_close_below_stop_no_exit() {
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 54.0,
        entry_price: 50.0,
        bars_since_entry: 2,
        active_stop: 55.0,
        direction: Direction::Short,
    };
    assert!(
        !ExitRule::StopLoss.should_exit(&ctx, &store),
        "short close 54 < stop 55 should NOT exit"
    );
}

#[test]
fn stop_loss_nan_stop_no_exit() {
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 44.0,
        entry_price: 50.0,
        bars_since_entry: 3,
        active_stop: f32::NAN,
        direction: Direction::Long,
    };
    assert!(
        !ExitRule::StopLoss.should_exit(&ctx, &store),
        "NaN stop should never exit"
    );
}

// ---------------------------------------------------------------------------
// SmaCross
// ---------------------------------------------------------------------------

#[test]
fn sma_cross_close_below_sma_exits_long() {
    // 1 row, 1 col. SMA10 at (0,0) = 50.0
    let store = make_test_store(1, 1, &[(Indicator::Sma10, vec![50.0])]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 49.0, // below SMA10
        entry_price: 45.0,
        bars_since_entry: 5,
        active_stop: 40.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::SmaCross {
        sma: Indicator::Sma10,
    };
    assert!(
        rule.should_exit(&ctx, &store),
        "long close 49 < SMA10 50 should exit"
    );
}

#[test]
fn sma_cross_close_above_sma_no_exit_long() {
    let store = make_test_store(1, 1, &[(Indicator::Sma10, vec![50.0])]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 51.0,
        entry_price: 45.0,
        bars_since_entry: 5,
        active_stop: 40.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::SmaCross {
        sma: Indicator::Sma10,
    };
    assert!(
        !rule.should_exit(&ctx, &store),
        "long close 51 >= SMA10 50 should NOT exit"
    );
}

#[test]
fn sma_cross_close_equal_sma_no_exit_long() {
    // close == sma: NOT < sma, so no exit for long
    let store = make_test_store(1, 1, &[(Indicator::Sma10, vec![50.0])]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 50.0,
        entry_price: 45.0,
        bars_since_entry: 5,
        active_stop: 40.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::SmaCross {
        sma: Indicator::Sma10,
    };
    assert!(
        !rule.should_exit(&ctx, &store),
        "long close == SMA should NOT exit (strict <)"
    );
}

#[test]
fn sma_cross_nan_sma_no_exit() {
    // SMA is NAN -> no exit
    let store = make_test_store(1, 1, &[]); // all NaN
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 30.0,
        entry_price: 45.0,
        bars_since_entry: 5,
        active_stop: 40.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::SmaCross {
        sma: Indicator::Sma10,
    };
    assert!(
        !rule.should_exit(&ctx, &store),
        "NaN SMA should never trigger exit"
    );
}

// ---------------------------------------------------------------------------
// ProfitTarget
// ---------------------------------------------------------------------------

#[test]
fn profit_target_bars_met_and_profitable_exits() {
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 55.0, // above entry
        entry_price: 50.0,
        bars_since_entry: 5,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::ProfitTarget { min_bars: 5 };
    assert!(
        rule.should_exit(&ctx, &store),
        "bars=5, close > entry should exit"
    );
}

#[test]
fn profit_target_bars_not_met_no_exit() {
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 55.0,
        entry_price: 50.0,
        bars_since_entry: 4, // not enough bars
        active_stop: 45.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::ProfitTarget { min_bars: 5 };
    assert!(
        !rule.should_exit(&ctx, &store),
        "bars=4 < min_bars=5 should NOT exit"
    );
}

#[test]
fn profit_target_not_profitable_no_exit() {
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 48.0, // below entry
        entry_price: 50.0,
        bars_since_entry: 10,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::ProfitTarget { min_bars: 5 };
    assert!(
        !rule.should_exit(&ctx, &store),
        "close < entry should NOT exit even with enough bars"
    );
}

#[test]
fn profit_target_close_equals_entry_no_exit() {
    // close == entry: NOT > entry, so no exit
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 50.0,
        entry_price: 50.0,
        bars_since_entry: 10,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::ProfitTarget { min_bars: 5 };
    assert!(
        !rule.should_exit(&ctx, &store),
        "close == entry is not strictly profitable"
    );
}

// ---------------------------------------------------------------------------
// BreakevenUpgrade
// ---------------------------------------------------------------------------

#[test]
fn breakeven_upgrade_never_triggers_exit() {
    let store = make_test_store(1, 1, &[]);
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 60.0,
        entry_price: 50.0,
        bars_since_entry: 10,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::BreakevenUpgrade { after_bars: 5 };
    assert!(
        !rule.should_exit(&ctx, &store),
        "BreakevenUpgrade should NEVER trigger an exit"
    );
}

#[test]
fn breakeven_upgrade_updates_stop_when_conditions_met() {
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 55.0,
        entry_price: 50.0,
        bars_since_entry: 5,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::BreakevenUpgrade { after_bars: 5 };
    let new_stop = rule.update_stop(&ctx);

    // bars >= 5 and close > entry -> upgrade stop to max(45, 50) = 50 (entry)
    assert!(new_stop.is_some(), "should return Some(new_stop)");
    assert!(
        (new_stop.unwrap() - 50.0).abs() < 1e-6,
        "new stop should be entry_price=50, got {}",
        new_stop.unwrap(),
    );
}

#[test]
fn breakeven_upgrade_keeps_higher_stop() {
    // If active_stop is already above entry, keep the higher one
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 60.0,
        entry_price: 50.0,
        bars_since_entry: 10,
        active_stop: 52.0, // already above entry
        direction: Direction::Long,
    };
    let rule = ExitRule::BreakevenUpgrade { after_bars: 5 };
    let new_stop = rule.update_stop(&ctx);
    assert!(new_stop.is_some());
    assert!(
        (new_stop.unwrap() - 52.0).abs() < 1e-6,
        "should keep higher existing stop=52, got {}",
        new_stop.unwrap(),
    );
}

#[test]
fn breakeven_upgrade_too_early_no_update() {
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 55.0,
        entry_price: 50.0,
        bars_since_entry: 4, // not enough bars
        active_stop: 45.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::BreakevenUpgrade { after_bars: 5 };
    assert!(
        rule.update_stop(&ctx).is_none(),
        "bars < after_bars should return None"
    );
}

#[test]
fn breakeven_upgrade_not_profitable_no_update() {
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 48.0, // below entry
        entry_price: 50.0,
        bars_since_entry: 10,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::BreakevenUpgrade { after_bars: 5 };
    assert!(
        rule.update_stop(&ctx).is_none(),
        "not profitable should return None"
    );
}

// ---------------------------------------------------------------------------
// SmaCover
// ---------------------------------------------------------------------------

#[test]
fn sma_cover_close_below_first_sma_exits() {
    // close=49 <= SMA10=50 -> exit
    let store = make_test_store(
        1,
        1,
        &[
            (Indicator::Sma10, vec![50.0]),
            (Indicator::Sma20, vec![45.0]),
        ],
    );
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 49.0,
        entry_price: 55.0,
        bars_since_entry: 5,
        active_stop: 60.0,
        direction: Direction::Short,
    };
    let rule = ExitRule::SmaCover {
        sma1: Indicator::Sma10,
        sma2: Indicator::Sma20,
    };
    assert!(
        rule.should_exit(&ctx, &store),
        "close 49 <= SMA10 50 should trigger cover"
    );
}

#[test]
fn sma_cover_close_below_second_sma_exits() {
    // close=44 <= SMA20=45 (but above SMA10=40) -> exit
    let store = make_test_store(
        1,
        1,
        &[
            (Indicator::Sma10, vec![40.0]),
            (Indicator::Sma20, vec![45.0]),
        ],
    );
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 44.0,
        entry_price: 55.0,
        bars_since_entry: 3,
        active_stop: 60.0,
        direction: Direction::Short,
    };
    let rule = ExitRule::SmaCover {
        sma1: Indicator::Sma10,
        sma2: Indicator::Sma20,
    };
    assert!(
        rule.should_exit(&ctx, &store),
        "close 44 <= SMA20 45 should trigger cover"
    );
}

#[test]
fn sma_cover_close_above_both_smas_no_exit() {
    let store = make_test_store(
        1,
        1,
        &[
            (Indicator::Sma10, vec![40.0]),
            (Indicator::Sma20, vec![45.0]),
        ],
    );
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 46.0, // above both
        entry_price: 55.0,
        bars_since_entry: 3,
        active_stop: 60.0,
        direction: Direction::Short,
    };
    let rule = ExitRule::SmaCover {
        sma1: Indicator::Sma10,
        sma2: Indicator::Sma20,
    };
    assert!(
        !rule.should_exit(&ctx, &store),
        "close 46 above both SMAs should NOT trigger cover"
    );
}

#[test]
fn sma_cover_close_equals_sma_exits() {
    // close == SMA -> <= holds -> should exit
    let store = make_test_store(
        1,
        1,
        &[
            (Indicator::Sma10, vec![50.0]),
            (Indicator::Sma20, vec![45.0]),
        ],
    );
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 50.0,
        entry_price: 55.0,
        bars_since_entry: 3,
        active_stop: 60.0,
        direction: Direction::Short,
    };
    let rule = ExitRule::SmaCover {
        sma1: Indicator::Sma10,
        sma2: Indicator::Sma20,
    };
    assert!(
        rule.should_exit(&ctx, &store),
        "close == SMA10 should trigger cover (<= check)"
    );
}

#[test]
fn sma_cover_nan_smas_no_exit() {
    // Both SMAs are NaN -> no exit
    let store = make_test_store(1, 1, &[]); // all NaN
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 30.0,
        entry_price: 55.0,
        bars_since_entry: 3,
        active_stop: 60.0,
        direction: Direction::Short,
    };
    let rule = ExitRule::SmaCover {
        sma1: Indicator::Sma10,
        sma2: Indicator::Sma20,
    };
    assert!(
        !rule.should_exit(&ctx, &store),
        "NaN SMAs should never trigger cover"
    );
}

// ---------------------------------------------------------------------------
// update_stop returns None for non-BreakevenUpgrade rules
// ---------------------------------------------------------------------------

#[test]
fn stop_loss_update_stop_returns_none() {
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 55.0,
        entry_price: 50.0,
        bars_since_entry: 10,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    assert!(ExitRule::StopLoss.update_stop(&ctx).is_none());
}

#[test]
fn sma_cross_update_stop_returns_none() {
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 55.0,
        entry_price: 50.0,
        bars_since_entry: 10,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::SmaCross {
        sma: Indicator::Sma10,
    };
    assert!(rule.update_stop(&ctx).is_none());
}

#[test]
fn profit_target_update_stop_returns_none() {
    let ctx = ExitContext {
        row: 0,
        col: 0,
        close: 55.0,
        entry_price: 50.0,
        bars_since_entry: 10,
        active_stop: 45.0,
        direction: Direction::Long,
    };
    let rule = ExitRule::ProfitTarget { min_bars: 5 };
    assert!(rule.update_stop(&ctx).is_none());
}
