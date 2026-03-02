//! Signal pipeline wrapper block.
//!
//! Wraps the existing L0->L1->L2->L3 signal pipeline from `engine-signals`
//! as a single pipeline block. Operates per-column (per ticker), consuming a
//! universe mask and producing a `SignalSet` with entries, exits, and stop
//! prices.
//!
//! Currently stubbed: `engine-signals` is not yet wired as a dependency of
//! `engine-pipeline`. The full implementation (mirroring
//! `signal_breakout_signals()` in `src/strategy/signal_breakout.rs`) will be
//! connected in Wave 3 integration.

use crate::blackboard::{Slot, SlotType};
use super::{Block, BlockContext};

/// Wraps the L0->L3 signal characterization pipeline as a single block.
///
/// Config keys (all optional, use signal defaults when absent):
/// - `"candidate_threshold"`: f32 -- minimum score for entry candidacy
/// - `"window_len"`: usize -- lookback window for characterization
/// - `"min_signal_data"`: usize -- minimum data points per ticker (default 30)
/// - `"stop_fallback_pct"`: f32 -- fallback stop as pct below entry (default 0.05)
pub struct SignalPipelineBlock;

impl Block for SignalPipelineBlock {
    fn name(&self) -> &'static str { "signal_pipeline" }
    fn output_type(&self) -> SlotType { SlotType::Signals }

    fn input_type(&self) -> Option<SlotType> {
        Some(SlotType::Mask)
    }

    fn execute(&self, _ctx: &BlockContext) -> anyhow::Result<Slot> {
        // TODO: Wire engine-signals dependency and implement full pipeline.
        //
        // The implementation should mirror signal_breakout.rs:69-182:
        //   1. Read universe mask from input
        //   2. For each active column (ticker), extract close/indicator slices
        //   3. Run characterize() from engine_signals::pipeline
        //   4. Run compute_score() from engine_signals::scorer
        //   5. Apply candidate_threshold to determine entries
        //   6. Compute stop prices (ATR-capped low or fallback_pct)
        //   7. Build exits from BOCPD/Kalman exit signals
        //   8. Return SignalSet { entries, exits, stop_prices }
        //
        // Internal state (ThreadArena) is created per-execute call since blocks
        // are stateless.
        anyhow::bail!(
            "signal_pipeline block requires engine-signals crate \
             (add to engine-pipeline/Cargo.toml, will be wired in Wave 3)"
        )
    }
}
