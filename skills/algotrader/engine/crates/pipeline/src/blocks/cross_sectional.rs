//! Cross-sectional gate blocks (stubs).
//!
//! These blocks compute per-row (per-date) scalar values derived from
//! cross-sectional properties of the full ticker universe. They are used as
//! regime gates: high susceptibility or low entropy can signal crowded/fragile
//! markets where breakout strategies underperform.
//!
//! All three are currently stubs returning all-pass values (1.0 for every row).
//! The real algorithms will be implemented when the pipeline is integrated with
//! the full correlation and Ising model infrastructure.

use crate::blackboard::{Slot, SlotType};
use super::{Block, BlockContext};

// ---------------------------------------------------------------------------
// IsingSusceptibility
// ---------------------------------------------------------------------------

/// Ising model susceptibility: measures herding/crowding in the market.
///
/// TODO: Implementation plan:
///   1. Map each ticker to spin +1/-1 based on sign of daily return.
///   2. Compute magnetization M(t) = mean(spins) per row.
///   3. Susceptibility = variance(M) over a rolling window (e.g. 20 bars).
///   4. High susceptibility => tickers are moving in lockstep (herding),
///      which precedes regime breaks and makes breakout signals unreliable.
pub struct IsingSusceptibility;

impl Block for IsingSusceptibility {
    fn name(&self) -> &'static str { "ising_susceptibility" }
    fn output_type(&self) -> SlotType { SlotType::Scalar }

    fn input_type(&self) -> Option<SlotType> {
        // Reads directly from DataStore, no mask input required.
        None
    }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let nr = ctx.store.axes.n_rows;
        // Stub: susceptibility always "high" (all rows pass any downstream gate).
        Ok(Slot::Scalar(vec![1.0; nr]))
    }
}

// ---------------------------------------------------------------------------
// VnEntropy
// ---------------------------------------------------------------------------

/// Von Neumann entropy of the return correlation matrix.
///
/// TODO: Implementation plan:
///   1. Compute rolling correlation matrix of returns (e.g. 60-bar window).
///   2. Normalize as a density matrix: rho = C / trace(C).
///   3. Eigendecompose rho to get eigenvalues lambda_i.
///   4. Entropy S = -sum(lambda_i * ln(lambda_i)) for lambda_i > 0.
///   5. Low entropy => market dominated by few factors (risky for diversified
///      breakout strategies). High entropy => returns are spread across many
///      independent modes (healthier for stock-picking).
pub struct VnEntropy;

impl Block for VnEntropy {
    fn name(&self) -> &'static str { "vn_entropy" }
    fn output_type(&self) -> SlotType { SlotType::Scalar }

    fn input_type(&self) -> Option<SlotType> {
        None
    }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let nr = ctx.store.axes.n_rows;
        // Stub: entropy always "high" (all rows pass).
        Ok(Slot::Scalar(vec![1.0; nr]))
    }
}

// ---------------------------------------------------------------------------
// Quorum
// ---------------------------------------------------------------------------

/// Quorum gate: requires a minimum fraction of tickers to be candidates.
///
/// TODO: Implementation plan:
///   1. Read a per-cell score from a prior step (config `"score_input"` step ID).
///   2. Count tickers above `"candidate_threshold"` (f32) per row.
///   3. Compute fraction = count / total_tickers.
///   4. If fraction >= `"quorum_threshold"` (f32), the row passes (value 1.0).
///      Otherwise the row is gated (value 0.0).
///   5. This prevents trading on days when very few tickers are actionable,
///      which often indicates adverse market conditions.
///
/// Config:
///   - `"score_input"`: step ID producing a Scalar or Matrix with per-cell scores
///   - `"candidate_threshold"`: f32 -- minimum score for a ticker to count
///   - `"quorum_threshold"`: f32 -- minimum fraction of qualifying tickers
pub struct Quorum;

impl Block for Quorum {
    fn name(&self) -> &'static str { "quorum" }
    fn output_type(&self) -> SlotType { SlotType::Scalar }

    fn input_type(&self) -> Option<SlotType> {
        None
    }

    fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
        let nr = ctx.store.axes.n_rows;
        // Stub: quorum always met (all rows pass).
        Ok(Slot::Scalar(vec![1.0; nr]))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use engine_data::{DataStore, Axes};
    use engine_types::WideMatrix;
    use crate::blackboard::Blackboard;

    /// Matches strum EnumCount for Indicator without adding strum as a dep.
    const INDICATOR_COUNT: usize = 26;

    /// Build a minimal DataStore with the given number of rows and 1 column.
    fn dummy_store(n_rows: usize) -> DataStore {
        let nc = 1;
        let daily: Vec<WideMatrix> = (0..INDICATOR_COUNT)
            .map(|_| WideMatrix::new(vec![0.0; n_rows * nc], n_rows, nc))
            .collect();
        let axes = Axes {
            dates: vec![0; n_rows],
            tickers: vec!["A".into()],
            ticker_idx: [("A".into(), 0)].into(),
            spy_col: None,
            etf_cols: vec![false; nc],
            n_rows,
            n_cols: nc,
            trading_hours: 6.5,
        };
        DataStore::new(axes, daily, None)
    }

    /// All three stubs should return a Scalar of all 1.0 values.
    fn run_stub(block: &dyn Block, n_rows: usize) -> Vec<f32> {
        let store = dummy_store(n_rows);
        let bb = Blackboard::new();
        let ctx = BlockContext {
            store: &store,
            config: &HashMap::new(),
            range: 0..n_rows,
            blackboard: &bb,
            input_id: None,
        };
        let slot = block.execute(&ctx).unwrap();
        slot.as_scalar().unwrap().clone()
    }

    #[test]
    fn ising_stub_all_pass() {
        let vals = run_stub(&IsingSusceptibility, 5);
        assert_eq!(vals, vec![1.0; 5]);
    }

    #[test]
    fn vn_entropy_stub_all_pass() {
        let vals = run_stub(&VnEntropy, 3);
        assert_eq!(vals, vec![1.0; 3]);
    }

    #[test]
    fn quorum_stub_all_pass() {
        let vals = run_stub(&Quorum, 4);
        assert_eq!(vals, vec![1.0; 4]);
    }
}
