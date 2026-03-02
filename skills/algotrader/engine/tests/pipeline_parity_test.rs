//! Parity tests: hardcoded Qullamaggie strategies vs dynamic JSON pipeline.
//!
//! For each strategy (EP, Parabolic), builds a synthetic DataStore with known
//! indicator values, runs BOTH the hardcoded Setup and the DynamicSetup loaded
//! from JSON, and asserts filter masks and signal sets are bit-identical.
//!
//! Breakout parity is tested separately because the hardcoded breakout combines
//! filter + signal logic (including pattern detection and volume spike) in a way
//! that requires the full signal phase, not just the filter phase.

use std::collections::HashMap;
use strum::EnumCount;

use algotrader_engine::data::{Axes, DataStore, Indicator, WideMatrix};
use algotrader_engine::strategy::{EpisodicPivot, ParabolicShort, Setup};
use algotrader_engine::types::{Params, SignalSet, WideMask};

// ---------------------------------------------------------------------------
// Test DataStore builder
// ---------------------------------------------------------------------------

/// Create a DataStore with `n_rows` x `n_cols` shape. All matrices default to
/// NaN. Only the (Indicator, data) pairs in `values` are overwritten.
fn make_store(
    n_rows: usize,
    n_cols: usize,
    values: &[(Indicator, Vec<f32>)],
) -> DataStore {
    let mut daily: Vec<WideMatrix> = (0..Indicator::COUNT)
        .map(|_| WideMatrix::new(vec![f32::NAN; n_rows * n_cols], n_rows, n_cols))
        .collect();

    for (ind, data) in values {
        assert_eq!(
            data.len(),
            n_rows * n_cols,
            "indicator {:?} data length mismatch: expected {}, got {}",
            ind,
            n_rows * n_cols,
            data.len()
        );
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

/// Helper to set a single cell value in a flat matrix.
fn set(data: &mut [f32], n_cols: usize, row: usize, col: usize, val: f32) {
    data[row * n_cols + col] = val;
}

// ---------------------------------------------------------------------------
// Mask/signal comparison helpers
// ---------------------------------------------------------------------------

fn assert_masks_equal(label: &str, a: &WideMask, b: &WideMask) {
    assert_eq!(a.data.len(), b.data.len(), "{label}: mask length mismatch");
    for i in 0..a.data.len() {
        assert_eq!(
            a.data[i], b.data[i],
            "{label}: mismatch at flat index {i} (a={}, b={})",
            a.data[i], b.data[i]
        );
    }
}

fn assert_stops_equal(label: &str, a: &WideMatrix, b: &WideMatrix) {
    assert_eq!(a.data.len(), b.data.len(), "{label}: stop array length mismatch");
    for i in 0..a.data.len() {
        let va = a.data[i];
        let vb = b.data[i];
        if va.is_nan() && vb.is_nan() {
            continue;
        }
        assert!(
            (va - vb).abs() < 1e-4,
            "{label}: stop mismatch at flat index {i} (a={va}, b={vb})"
        );
    }
}

fn assert_signals_equal(label: &str, a: &SignalSet, b: &SignalSet) {
    assert_masks_equal(&format!("{label} entries"), &a.entries, &b.entries);
    assert_masks_equal(&format!("{label} exits"), &a.exits, &b.exits);
    assert_stops_equal(&format!("{label} stops"), &a.stop_prices, &b.stop_prices);
}

// ---------------------------------------------------------------------------
// EP (Episodic Pivot) parity test
// ---------------------------------------------------------------------------

/// Build a DataStore where specific cells are designed to pass/fail the EP
/// filter, allowing us to verify hardcoded vs dynamic produce identical results.
///
/// Layout: 10 rows x 3 cols
/// - Col 0: passes all EP filters at row 5 (gap-up day)
/// - Col 1: fails due to gap being too small
/// - Col 2: fails due to high prior 6M return (not neglected)
fn make_ep_store() -> DataStore {
    let nr = 10;
    let nc = 3;
    let n = nr * nc;

    // Close: all at 50.0 (above min_price=5.0)
    let mut close = vec![50.0; n];
    // Open: same as close by default
    let mut open = vec![50.0; n];
    // Volume: all at 500000 (above min_vol=300000)
    // VolSma20: need vsma * close >= min_adv (150M). So vsma >= 150M/50 = 3M.
    let vol_sma = vec![3_100_000.0; n];
    // Ret126: default 0.10 (below max_prior_6m_return=0.30) except col 2
    let mut ret_126 = vec![0.10; n];
    // Low: used for stops
    let low = vec![48.0; n];
    // Sma10: 49.0 (close 50.0 > sma10 49.0, so no exit signal for most cells)
    let sma10 = vec![49.0; n];

    // Row 4 close (prev_close for gap calc at row 5)
    for col in 0..nc {
        set(&mut close, nc, 4, col, 40.0); // prev_close = 40.0
    }

    // Row 5: gap day
    // Col 0: open=48.0 → gap = 48/40 - 1 = 0.20 (> min_gap_pct=0.10) ✓
    //         vol=500000 > min_vol_ratio(2.0) * vsma(3.1M)? 500k < 6.2M → FAIL
    //         Need vol on gap day to be high enough: vol >= 2.0 * 3.1M = 6.2M
    // Let me use actual volume for gap day
    // Actually, the vol array is flat. Let me use a separate approach.

    // Rethink: I need actual volume at row 5 to spike. But the flat vol array
    // applies everywhere. I need per-cell control.

    // Let me rebuild with more careful per-cell data.
    let mut vol_data = vec![500000.0; n]; // base volume everywhere

    // Gap day (row 5): need vol >= min_vol_ratio * vol_sma = 2.0 * 3.1M = 6.2M
    set(&mut vol_data, nc, 5, 0, 7_000_000.0);
    set(&mut vol_data, nc, 5, 1, 7_000_000.0);
    set(&mut vol_data, nc, 5, 2, 7_000_000.0);

    // Col 0, row 5: open=48.0 → gap = 48/40 - 1 = 0.20 ✓
    set(&mut open, nc, 5, 0, 48.0);
    set(&mut close, nc, 5, 0, 52.0);

    // Col 1, row 5: open=42.0 → gap = 42/40 - 1 = 0.05 (< 0.10) ✗
    set(&mut open, nc, 5, 1, 42.0);
    set(&mut close, nc, 5, 1, 52.0);

    // Col 2, row 5: open=48.0 → gap = 48/40 - 1 = 0.20 ✓ (but ret_126 will block)
    set(&mut open, nc, 5, 2, 48.0);
    set(&mut close, nc, 5, 2, 52.0);

    // Col 2: ret_126 = 0.50 (> max_prior_6m_return=0.30) → fails neglect filter
    for row in 0..nr {
        set(&mut ret_126, nc, row, 2, 0.50);
    }

    // Dollar volume check: vol * close >= min_dollar_vol (100M)
    // Row 5, col 0: 7M * 52 = 364M ✓
    // Row 5, col 1: 7M * 52 = 364M ✓ (but gap too small)
    // Row 5, col 2: 7M * 52 = 364M ✓ (but ret_126 blocks)

    make_store(nr, nc, &[
        (Indicator::Close, close),
        (Indicator::Open, open),
        (Indicator::Volume, vol_data),
        (Indicator::VolSma20, vol_sma),
        (Indicator::Ret126, ret_126),
        (Indicator::Low, low),
        (Indicator::Sma10, sma10),
    ])
}

#[test]
fn ep_filter_parity() {
    let store = make_ep_store();
    let params = Params::default();
    let range = 0..store.axes.n_rows;

    // Hardcoded EP filter
    let hardcoded = EpisodicPivot;
    let hc_mask = hardcoded.filter(&store, &params, range.clone());

    // Dynamic EP from JSON
    let json = include_str!("../strategies/ep_dynamic.json");
    let dynamic = engine_pipeline::DynamicSetup::from_json(json).unwrap();
    let dyn_mask = dynamic.run_filter(&store, range).unwrap();

    assert_masks_equal("EP filter", &hc_mask, &dyn_mask);

    // Verify expected results: only col 0 at row 5 should pass
    let nc = store.axes.n_cols;
    assert!(hc_mask.get(5, 0), "col 0 row 5 should pass EP filter");
    assert!(!hc_mask.get(5, 1), "col 1 row 5 should fail (gap too small)");
    assert!(!hc_mask.get(5, 2), "col 2 row 5 should fail (high ret_126)");

    // No other rows should pass
    for row in 0..store.axes.n_rows {
        for col in 0..nc {
            if row == 5 && col == 0 {
                continue;
            }
            assert!(
                !hc_mask.get(row, col),
                "unexpected pass at row={row}, col={col}"
            );
        }
    }
}

#[test]
fn ep_signals_parity() {
    let store = make_ep_store();
    let params = Params::default();
    let range = 0..store.axes.n_rows;

    // Run filter first (both should agree)
    let hardcoded = EpisodicPivot;
    let hc_mask = hardcoded.filter(&store, &params, range.clone());

    // Run signals on the hardcoded filter mask
    let hc_signals = hardcoded.signals(&store, &hc_mask, &params);

    // Dynamic
    let json = include_str!("../strategies/ep_dynamic.json");
    let dynamic = engine_pipeline::DynamicSetup::from_json(json).unwrap();
    let dyn_mask = dynamic.run_filter(&store, range).unwrap();
    let dyn_signals = dynamic.run_signals(&store, &dyn_mask).unwrap();

    assert_signals_equal("EP signals", &hc_signals, &dyn_signals);

    // Entry should be at row 6 (shifted from gap day row 5), col 0
    assert!(hc_signals.entries.get(6, 0), "EP entry should be at row 6, col 0");
    assert!(!hc_signals.entries.get(5, 0), "EP entry should NOT be at gap day row 5");

    // Stop should be gap day's low = 48.0
    let nc = store.axes.n_cols;
    let stop = hc_signals.stop_prices.data[6 * nc + 0];
    assert!(
        (stop - 48.0).abs() < 1e-4,
        "EP stop should be gap day low (48.0), got {stop}"
    );
}

// ---------------------------------------------------------------------------
// Parabolic Short parity test
// ---------------------------------------------------------------------------

/// Build a DataStore for parabolic short parity.
///
/// Layout: 10 rows x 3 cols
/// - Col 0 (large cap, price > $50): passes at row 7 (first red after parabolic run)
/// - Col 1 (small cap, price < $50): fails (10d return too small for small cap threshold)
/// - Col 2 (large cap): fails (no consecutive green days before red)
fn make_parabolic_store() -> DataStore {
    let nr = 10;
    let nc = 3;
    let n = nr * nc;

    // Base: all prices at 60.0 (above min_price=5.0, above large_cap_price=50.0)
    let mut close = vec![60.0; n];
    let mut open = vec![59.0; n]; // green by default (close > open)
    let vol_sma = vec![3_100_000.0; n]; // vsma*close = 3.1M*60 = 186M > min_adv(150M) ✓
    let mut pct_10d = vec![0.10; n]; // default: not a parabolic run
    let mut consec_green = vec![0.0; n]; // default: no consecutive green days
    let high = vec![62.0; n]; // for stops
    let sma10 = vec![58.0; n]; // close 60 > sma10 58: no exit
    let sma20 = vec![56.0; n]; // close 60 > sma20 56: no exit

    // Col 0: large cap parabolic run
    // Need pct_10d >= large_cap_run (0.50) at row 7
    set(&mut pct_10d, nc, 7, 0, 0.60);
    // Row 7 is a red day: close < open
    set(&mut close, nc, 7, 0, 58.0);
    set(&mut open, nc, 7, 0, 63.0);
    // Prior day (row 6) had 3+ consecutive green days
    set(&mut consec_green, nc, 6, 0, 4.0);

    // Col 1: small cap (price = 30.0 < large_cap_price=50.0)
    // pct_10d = 0.60 → needs >= small_cap_run (3.00) for small cap → FAIL
    for row in 0..nr {
        set(&mut close, nc, row, 1, 30.0);
        set(&mut open, nc, row, 1, 29.0);
    }
    set(&mut pct_10d, nc, 7, 1, 0.60);
    set(&mut close, nc, 7, 1, 28.0); // red day
    set(&mut open, nc, 7, 1, 33.0);
    set(&mut consec_green, nc, 6, 1, 4.0);

    // Col 2: large cap, parabolic run, red day, but NO green days before
    set(&mut pct_10d, nc, 7, 2, 0.60);
    set(&mut close, nc, 7, 2, 58.0);
    set(&mut open, nc, 7, 2, 63.0);
    set(&mut consec_green, nc, 6, 2, 1.0); // only 1 green day (< 3)

    make_store(nr, nc, &[
        (Indicator::Close, close),
        (Indicator::Open, open),
        (Indicator::VolSma20, vol_sma),
        (Indicator::Pct10d, pct_10d),
        (Indicator::ConsecGreen, consec_green),
        (Indicator::High, high),
        (Indicator::Sma10, sma10),
        (Indicator::Sma20, sma20),
    ])
}

#[test]
fn parabolic_filter_parity() {
    let store = make_parabolic_store();
    let params = Params::default();
    let range = 0..store.axes.n_rows;

    let hardcoded = ParabolicShort;
    let hc_mask = hardcoded.filter(&store, &params, range.clone());

    let json = include_str!("../strategies/parabolic_dynamic.json");
    let dynamic = engine_pipeline::DynamicSetup::from_json(json).unwrap();
    let dyn_mask = dynamic.run_filter(&store, range).unwrap();

    assert_masks_equal("Parabolic filter", &hc_mask, &dyn_mask);

    // Only col 0 row 7 should pass
    assert!(hc_mask.get(7, 0), "col 0 row 7 should pass parabolic filter");
    assert!(!hc_mask.get(7, 1), "col 1 should fail (small cap, run too small)");
    assert!(!hc_mask.get(7, 2), "col 2 should fail (no green streak)");
}

#[test]
fn parabolic_signals_parity() {
    let store = make_parabolic_store();
    let params = Params::default();
    let range = 0..store.axes.n_rows;

    let hardcoded = ParabolicShort;
    let hc_mask = hardcoded.filter(&store, &params, range.clone());
    let hc_signals = hardcoded.signals(&store, &hc_mask, &params);

    let json = include_str!("../strategies/parabolic_dynamic.json");
    let dynamic = engine_pipeline::DynamicSetup::from_json(json).unwrap();
    let dyn_mask = dynamic.run_filter(&store, range).unwrap();
    let dyn_signals = dynamic.run_signals(&store, &dyn_mask).unwrap();

    assert_signals_equal("Parabolic signals", &hc_signals, &dyn_signals);

    // Entry at row 7, col 0
    assert!(hc_signals.entries.get(7, 0), "parabolic entry at row 7, col 0");

    // Stop should be high of entry day = 62.0
    let nc = store.axes.n_cols;
    let stop = hc_signals.stop_prices.data[7 * nc + 0];
    assert!(
        (stop - 62.0).abs() < 1e-4,
        "parabolic stop should be high (62.0), got {stop}"
    );
}
