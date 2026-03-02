//! Position sizing logic extracted from the 3x-duplicated block in simulate.rs.
//!
//! Computes shares and slippage-adjusted fill price using:
//! - Risk-based sizing (equity * risk_pct / price_risk)
//! - Max position cap (equity * max_pos_pct / fill_price)
//! - Liquidity cap (1% of average daily dollar volume / fill_price)
//! - Market-impact slippage model (sqrt of participation rate)

use crate::types::{Direction, Params};

pub struct PositionSizer {
    pub risk_pct: f32,
    pub max_pos_pct: f32,
    pub slippage_k: f32,
}

impl PositionSizer {
    pub fn from_params(params: &Params) -> Self {
        Self {
            risk_pct: params.risk_pct,
            max_pos_pct: params.max_pos_pct,
            slippage_k: params.slippage_k,
        }
    }

    /// Compute (shares, slippage-adjusted fill price).
    ///
    /// `equity` -- allocated capital for this position (already scaled by
    ///             equity_fraction for split-portfolio strategies).
    /// `fill_price` -- raw fill before slippage.
    /// `stop` -- initial stop price (used to derive price_risk).
    /// `adv` -- 20-day average volume (shares, not dollars).
    /// `close` -- current close price (for ADV dollar conversion).
    /// `direction` -- Long adds slippage, Short subtracts it.
    #[inline]
    pub fn compute(
        &self,
        equity: f64,
        fill_price: f32,
        stop: f32,
        adv: f32,
        close: f32,
        direction: Direction,
    ) -> (f32, f32) {
        let price_risk = (fill_price - stop).abs() as f64;
        // Safety: caller should have already verified price_risk > 0
        debug_assert!(price_risk > 0.0, "price_risk must be positive");

        let risk_shares = (equity * self.risk_pct as f64) / price_risk;
        let max_shares = (equity * self.max_pos_pct as f64) / fill_price as f64;

        let adv_dollar = adv as f64 * close as f64;
        let liquidity_cap = if adv_dollar > 0.0 {
            0.01 * adv_dollar / fill_price as f64
        } else {
            f64::MAX
        };

        let shares = risk_shares.min(max_shares).min(liquidity_cap).max(1.0) as f32;

        let slippage = if adv > 0.0 {
            (0.001 + self.slippage_k * (shares / adv).sqrt()).min(0.05)
        } else {
            0.001
        };

        let adjusted_fill = match direction {
            Direction::Long => fill_price * (1.0 + slippage),
            Direction::Short => fill_price * (1.0 - slippage),
        };

        (shares, adjusted_fill)
    }
}
