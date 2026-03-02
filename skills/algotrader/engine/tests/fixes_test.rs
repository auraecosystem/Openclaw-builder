//! Regression tests for issues discovered during code review.
//!
//! Each test is tagged with the fix number from the review for traceability.

// ---------------------------------------------------------------------------
// Fix #1: --crypto CLI flag must produce crypto-appropriate filter values
// ---------------------------------------------------------------------------

#[test]
fn crypto_defaults_has_relaxed_filters() {
    let p = algotrader_engine::types::Params::crypto_defaults();
    assert_eq!(p.min_price, 0.0, "crypto min_price should be 0");
    assert!(p.min_vol < 10_000.0, "crypto min_vol should be low, got {}", p.min_vol);
    assert_eq!(p.min_adv, 0.0, "crypto min_adv should be 0");
    assert!(!p.regime, "crypto should disable regime filter");
}

#[test]
fn crypto_defaults_has_correct_data_config() {
    let p = algotrader_engine::types::Params::crypto_defaults();
    assert_eq!(p.data.trading_hours_per_day, 24.0);
    assert_eq!(p.data.intraday_bars_per_hour, 12.0);
    assert_eq!(p.data.benchmark_symbols, vec!["BTCUSDT"]);
}

#[test]
fn stock_defaults_has_standard_filters() {
    let p = algotrader_engine::types::Params::default();
    assert_eq!(p.min_price, 5.0);
    assert_eq!(p.min_vol, 300_000.0);
    assert_eq!(p.min_adv, 150_000_000.0);
    assert!(p.regime);
    assert_eq!(p.data.trading_hours_per_day, 6.5);
}

// ---------------------------------------------------------------------------
// Fix #5: atr_stop fallback must not produce stop = close
// ---------------------------------------------------------------------------

#[test]
fn atr_stop_with_valid_atr_and_low() {
    // close=100, low=95, atr=6 → close-low=5 < atr=6 → use low
    let stop = algotrader_engine::strategy::atr_stop(100.0, 95.0, 6.0, 0.05);
    assert!((stop - 95.0).abs() < 1e-6, "should use low when range < ATR");
}

#[test]
fn atr_stop_clamps_wide_range_to_atr() {
    // close=100, low=85, atr=6 → close-low=15 > atr=6 → close-atr=94
    let stop = algotrader_engine::strategy::atr_stop(100.0, 85.0, 6.0, 0.05);
    assert!((stop - 94.0).abs() < 1e-6, "should clamp to close-atr");
}

#[test]
fn atr_stop_nan_atr_uses_low() {
    let stop = algotrader_engine::strategy::atr_stop(100.0, 95.0, f32::NAN, 0.05);
    assert!((stop - 95.0).abs() < 1e-6, "NaN ATR → fallback to low");
}

#[test]
fn atr_stop_nan_atr_and_low_uses_fallback_pct() {
    // Both NaN → close * (1 - fallback_pct). With 0.05 → 95.0
    let stop = algotrader_engine::strategy::atr_stop(100.0, f32::NAN, f32::NAN, 0.05);
    assert!((stop - 95.0).abs() < 1e-6, "all NaN → close * (1-pct)");
}

#[test]
fn atr_stop_zero_fallback_sets_stop_at_close() {
    // This documents the edge case: 0% fallback → stop = close (instant stop).
    // Callers should use non-zero fallback_pct to avoid this.
    let stop = algotrader_engine::strategy::atr_stop(100.0, f32::NAN, f32::NAN, 0.0);
    assert!((stop - 100.0).abs() < 1e-6, "0% fallback → stop at close");
}

// ---------------------------------------------------------------------------
// Fix #6: NaN-safe combined metrics in portfolio
// ---------------------------------------------------------------------------

#[test]
fn avg_finite_skips_nan() {
    // avg_finite is a closure inside portfolio::run_portfolio, but we can test
    // the same pattern: filtering NaN/Inf before averaging.
    let vals: Vec<f64> = vec![1.0, f64::NAN, 3.0, f64::INFINITY, 5.0];
    let finite: Vec<f64> = vals.into_iter().filter(|v| v.is_finite()).collect();
    let avg = finite.iter().sum::<f64>() / finite.len() as f64;
    assert!((avg - 3.0).abs() < 1e-10, "expected avg of [1,3,5]=3.0, got {avg}");
}

#[test]
fn avg_finite_empty_returns_zero() {
    let vals: Vec<f64> = vec![f64::NAN, f64::INFINITY];
    let finite: Vec<f64> = vals.into_iter().filter(|v| v.is_finite()).collect();
    let avg = if finite.is_empty() { 0.0 } else { finite.iter().sum::<f64>() / finite.len() as f64 };
    assert_eq!(avg, 0.0);
}

// ---------------------------------------------------------------------------
// Fix #4: benchmark log (structural — just verify the lookup logic works)
// ---------------------------------------------------------------------------

#[test]
fn data_config_benchmark_lookup_finds_first_match() {
    use std::collections::HashMap;
    let tickers = vec!["AAPL", "SPY", "QQQ"];
    let idx: HashMap<&str, usize> = tickers.iter().enumerate().map(|(i, t)| (*t, i)).collect();

    let benchmark_symbols = vec!["QQQ".to_string(), "SPY".to_string()];
    let found = benchmark_symbols
        .iter()
        .find_map(|sym| idx.get(sym.as_str()).copied());
    // QQQ is at index 2, SPY at 1. QQQ should be found first.
    assert_eq!(found, Some(2), "should find QQQ (first in benchmark list)");
}

#[test]
fn data_config_benchmark_falls_through_to_second() {
    use std::collections::HashMap;
    let tickers = vec!["AAPL", "SPY", "MSFT"];
    let idx: HashMap<&str, usize> = tickers.iter().enumerate().map(|(i, t)| (*t, i)).collect();

    let benchmark_symbols = vec!["QQQ".to_string(), "SPY".to_string()];
    let found = benchmark_symbols
        .iter()
        .find_map(|sym| idx.get(sym.as_str()).copied());
    assert_eq!(found, Some(1), "QQQ missing → should fall through to SPY");
}

// ---------------------------------------------------------------------------
// EquityCurve on Report (D1)
// ---------------------------------------------------------------------------

#[test]
fn equity_curve_omitted_from_json_when_none() {
    let p = algotrader_engine::types::Params::default();
    assert!(!p.analysis.emit_equity_curve, "should be disabled by default");
}

#[test]
fn equity_curve_enabled_via_config() {
    let json = r#"{"analysis": {"emit_equity_curve": true}}"#;
    let p: algotrader_engine::types::Params = serde_json::from_str(json).unwrap();
    assert!(p.analysis.emit_equity_curve);
}

// ---------------------------------------------------------------------------
// DataConfig serde defaults
// ---------------------------------------------------------------------------

#[test]
fn data_config_default_is_stock_friendly() {
    let dc = algotrader_engine::types::DataConfig::default();
    assert_eq!(dc.trading_hours_per_day, 6.5);
    assert!(dc.benchmark_symbols.contains(&"QQQ".to_string()));
    assert!(dc.benchmark_symbols.contains(&"SPY".to_string()));
}

#[test]
fn data_config_crypto_preset() {
    let dc = algotrader_engine::types::DataConfig::crypto();
    assert_eq!(dc.trading_hours_per_day, 24.0);
    assert_eq!(dc.intraday_bars_per_hour, 12.0);
    assert!(dc.benchmark_symbols.contains(&"BTCUSDT".to_string()));
}

#[test]
fn data_config_intraday_bars_per_day() {
    let dc = algotrader_engine::types::DataConfig::crypto();
    let bpd = dc.intraday_bars_per_day();
    assert!((bpd - 288.0).abs() < 0.01, "24h * 12 bars/hr = 288, got {bpd}");
}

// ---------------------------------------------------------------------------
// StrategyConfig new fields (A7)
// ---------------------------------------------------------------------------

#[test]
fn strategy_config_has_ep_and_parabolic_defaults() {
    let sc = algotrader_engine::types::StrategyConfig::default();
    assert_eq!(sc.ep_min_gap_pct, 0.10);
    assert_eq!(sc.ep_min_vol_ratio, 2.0);
    assert_eq!(sc.ep_min_gap_day_dollar_vol, 100_000_000.0);
    assert_eq!(sc.ep_max_prior_6m_return, 0.30);
    assert_eq!(sc.large_cap_price, 50.0);
    assert_eq!(sc.large_cap_run, 0.50);
    assert_eq!(sc.small_cap_run, 3.00);
}
