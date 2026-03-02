//! Unit tests for `algotrader_engine::execution::PositionSizer::compute()`.
//!
//! Validates the extracted position sizing logic: risk-based sizing, liquidity
//! cap, max position cap, slippage model, direction handling, and edge cases.

use algotrader_engine::execution::PositionSizer;
use algotrader_engine::types::Direction;

const EPS_F32: f32 = 1e-4;

/// Helper: build a PositionSizer with the default parameter values from Params.
fn default_sizer() -> PositionSizer {
    PositionSizer {
        risk_pct: 0.005,
        max_pos_pct: 0.20,
        slippage_k: 0.1,
    }
}

// ---------------------------------------------------------------------------
// Normal case
// ---------------------------------------------------------------------------

#[test]
fn normal_long_position() {
    let sizer = default_sizer();
    let equity = 100_000.0;
    let fill = 50.0_f32;
    let stop = 45.0_f32;
    let adv = 1_000_000.0_f32; // 1M shares avg daily volume
    let close = 50.0_f32;

    let (shares, adjusted_fill) = sizer.compute(equity, fill, stop, adv, close, Direction::Long);

    // price_risk = |50 - 45| = 5.0
    // risk_shares = (100000 * 0.005) / 5.0 = 100
    // max_shares  = (100000 * 0.20) / 50.0 = 400
    // adv_dollar  = 1_000_000 * 50 = 50_000_000
    // liq_cap     = 0.01 * 50_000_000 / 50.0 = 10_000
    // shares = min(100, 400, 10000).max(1) = 100
    assert!(
        (shares - 100.0).abs() < EPS_F32,
        "normal shares: got {}, expected 100",
        shares,
    );

    // slippage = 0.001 + 0.1 * sqrt(100 / 1_000_000)
    //          = 0.001 + 0.1 * sqrt(0.0001)
    //          = 0.001 + 0.1 * 0.01
    //          = 0.001 + 0.001 = 0.002
    // adjusted = 50.0 * (1 + 0.002) = 50.10
    let expected_slippage = 0.001 + 0.1 * (100.0_f32 / 1_000_000.0).sqrt();
    let expected_fill = fill * (1.0 + expected_slippage);
    assert!(
        (adjusted_fill - expected_fill).abs() < EPS_F32,
        "normal fill: got {}, expected {}",
        adjusted_fill,
        expected_fill,
    );
}

// ---------------------------------------------------------------------------
// Liquidity capped
// ---------------------------------------------------------------------------

#[test]
fn liquidity_capped_low_adv() {
    let sizer = default_sizer();
    let equity = 100_000.0;
    let fill = 50.0_f32;
    let stop = 45.0_f32;
    let adv = 100.0_f32; // Only 100 shares avg daily volume
    let close = 50.0_f32;

    let (shares, _) = sizer.compute(equity, fill, stop, adv, close, Direction::Long);

    // price_risk = 5.0
    // risk_shares = 100
    // max_shares  = 400
    // adv_dollar  = 100 * 50 = 5000
    // liq_cap     = 0.01 * 5000 / 50 = 1.0
    // shares = min(100, 400, 1.0).max(1) = 1.0
    assert!(
        (shares - 1.0).abs() < EPS_F32,
        "liq_cap shares: got {}, expected 1.0",
        shares,
    );
}

// ---------------------------------------------------------------------------
// Max position capped
// ---------------------------------------------------------------------------

#[test]
fn max_position_capped() {
    let sizer = default_sizer();
    let equity = 100_000.0;
    let fill = 50.0_f32;
    let stop = 49.99_f32; // Tiny risk distance -> huge risk_shares
    let adv = 1_000_000.0_f32;
    let close = 50.0_f32;

    let (shares, _) = sizer.compute(equity, fill, stop, adv, close, Direction::Long);

    // price_risk = |50 - 49.99| = 0.01
    // risk_shares = (100000 * 0.005) / 0.01 = 50000
    // max_shares  = (100000 * 0.20) / 50.0 = 400
    // liq_cap     = 10000
    // shares = min(50000, 400, 10000).max(1) = 400
    assert!(
        (shares - 400.0).abs() < EPS_F32,
        "max_pos shares: got {}, expected 400",
        shares,
    );
}

// ---------------------------------------------------------------------------
// Short direction
// ---------------------------------------------------------------------------

#[test]
fn short_direction_slippage_reduces_fill() {
    let sizer = default_sizer();
    let equity = 100_000.0;
    let fill = 50.0_f32;
    let stop = 55.0_f32; // Short: stop is above entry
    let adv = 1_000_000.0_f32;
    let close = 50.0_f32;

    let (shares, adjusted_fill) = sizer.compute(equity, fill, stop, adv, close, Direction::Short);

    // price_risk = |50 - 55| = 5.0 -> risk_shares = 100
    assert!(
        (shares - 100.0).abs() < EPS_F32,
        "short shares: got {}, expected 100",
        shares,
    );

    // For shorts, slippage *reduces* the fill price (worse for short entry)
    let expected_slippage = 0.001 + 0.1 * (100.0_f32 / 1_000_000.0).sqrt();
    let expected_fill = fill * (1.0 - expected_slippage);
    assert!(
        (adjusted_fill - expected_fill).abs() < EPS_F32,
        "short fill: got {}, expected {}",
        adjusted_fill,
        expected_fill,
    );
}

// ---------------------------------------------------------------------------
// Zero ADV
// ---------------------------------------------------------------------------

#[test]
fn zero_adv_fixed_minimum_slippage() {
    let sizer = default_sizer();
    let equity = 100_000.0;
    let fill = 50.0_f32;
    let stop = 45.0_f32;
    let adv = 0.0_f32;
    let close = 50.0_f32;

    let (shares, adjusted_fill) = sizer.compute(equity, fill, stop, adv, close, Direction::Long);

    // adv_dollar = 0 -> liquidity_cap = f64::MAX (no constraint)
    // risk_shares = 100, max_shares = 400
    // shares = min(100, 400, MAX).max(1) = 100
    assert!(
        (shares - 100.0).abs() < EPS_F32,
        "zero_adv shares: got {}, expected 100",
        shares,
    );

    // adv=0 -> slippage = 0.001 (fixed minimum)
    let expected_fill = fill * (1.0 + 0.001);
    assert!(
        (adjusted_fill - expected_fill).abs() < EPS_F32,
        "zero_adv fill: got {}, expected {}",
        adjusted_fill,
        expected_fill,
    );
}

// ---------------------------------------------------------------------------
// Minimum 1 share
// ---------------------------------------------------------------------------

#[test]
fn min_one_share_very_high_price() {
    let sizer = default_sizer();
    let equity = 100_000.0;
    let fill = 999_999.0_f32; // Very expensive stock
    let stop = 999_998.0_f32; // 1.0 risk
    let adv = 1_000.0_f32;
    let close = 999_999.0_f32;

    let (shares, _) = sizer.compute(equity, fill, stop, adv, close, Direction::Long);

    // price_risk = 1.0
    // risk_shares = (100000 * 0.005) / 1.0 = 500
    // max_shares  = (100000 * 0.20) / 999999 ~= 0.02
    // adv_dollar  = 1000 * 999999 ~= 1e9
    // liq_cap     = 0.01 * 1e9 / 999999 ~= 10
    // shares = min(500, 0.02, 10).max(1) = 1.0  (floored to 1)
    assert!(
        (shares - 1.0).abs() < EPS_F32,
        "min_1_share: got {}, expected 1.0",
        shares,
    );
}

// ---------------------------------------------------------------------------
// Slippage cap at 5%
// ---------------------------------------------------------------------------

#[test]
fn slippage_capped_at_five_percent() {
    // When shares/adv ratio is very high, slippage should be capped at 0.05
    let sizer = PositionSizer {
        risk_pct: 0.005,
        max_pos_pct: 0.99, // Very high cap so shares are large
        slippage_k: 10.0,  // Very high slippage coefficient
    };
    let equity = 100_000_000.0; // $100M
    let fill = 10.0_f32;
    let stop = 9.0_f32;
    let adv = 10_000.0_f32;
    let close = 10.0_f32;

    let (_shares, adjusted_fill) = sizer.compute(equity, fill, stop, adv, close, Direction::Long);

    // With these extreme parameters, slippage formula yields >> 0.05,
    // but it should be capped at 0.05
    let expected_fill = fill * (1.0 + 0.05);
    assert!(
        adjusted_fill <= expected_fill + EPS_F32,
        "slippage cap: adjusted_fill={} should be <= {}",
        adjusted_fill,
        expected_fill,
    );
}
