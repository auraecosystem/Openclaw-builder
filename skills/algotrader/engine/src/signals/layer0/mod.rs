//! Layer 0 -- Data Characterization
//!
//! Recomputed infrequently (~every 100 bars) to set the regime context that
//! conditions all downstream scanner and agent decisions.

pub mod hmm;
pub mod hurst;
pub mod rmt;
pub mod transfer;

/// Snapshot of the current market characterization produced by Layer 0.
///
/// Carried between bar updates and only recomputed when
/// `bar_idx - last_updated >= l0_recompute_interval`.
#[derive(Clone, Debug)]
pub struct CharacterizationState {
    /// DFA Hurst exponent in [0, 1]. H > 0.55 = trending, H < 0.45 = mean-rev.
    pub hurst: f32,
    /// HMM regime state: 0 = low-vol, 1 = high-vol, 2 = trending.
    pub hmm_state: u8,
    /// RMT-filtered correlation matrix (N*N flattened row-major), if available.
    pub cleaned_corr: Option<Vec<f32>>,
    /// Transfer entropy scores (N values), if computed.
    pub te_scores: Option<Vec<f32>>,
    /// Bar index at which this state was last recomputed.
    pub last_updated: usize,
}

impl Default for CharacterizationState {
    fn default() -> Self {
        Self {
            hurst: 0.5,
            hmm_state: 0,
            cleaned_corr: None,
            te_scores: None,
            last_updated: 0,
        }
    }
}

/// Recompute Layer 0 characterization if enough bars have elapsed since the
/// previous computation.
///
/// Arguments:
/// - `close`: full close-price column for the current ticker.
/// - `returns`: log-return series for the current ticker.
/// - `all_returns`: optional flat row-major returns matrix for all tickers
///   `(data, n_tickers, n_bars)` used by RMT filtering.
/// - `bar_idx`: current bar index (monotonically increasing).
/// - `recompute_interval`: minimum bars between recomputations (from params).
/// - `hurst_window`: DFA window size (from params).
/// - `prev`: previous characterization state.
///
/// Returns a new `CharacterizationState`. If the recompute interval has not
/// elapsed, returns a clone of `prev` unchanged.
pub fn characterize(
    close: &[f32],
    returns: &[f32],
    all_returns: Option<(&[f32], usize, usize)>,
    bar_idx: usize,
    recompute_interval: usize,
    hurst_window: usize,
    prev: &CharacterizationState,
) -> CharacterizationState {
    // Not time yet -- return previous state.
    if bar_idx.saturating_sub(prev.last_updated) < recompute_interval {
        return prev.clone();
    }

    // --- Hurst exponent ---
    let h = if close.len() >= hurst_window {
        hurst::compute_hurst(close, hurst_window)
    } else {
        prev.hurst
    };

    // --- HMM regime state ---
    let hmm_s = hmm::compute_hmm_state(returns, &hmm::HmmModel::default());

    // --- RMT correlation filtering (multi-asset) ---
    let rmt_result = all_returns.map(|(r, nt, nb)| rmt::compute_rmt_filtered(r, nt, nb));

    CharacterizationState {
        hurst: h,
        hmm_state: hmm_s,
        cleaned_corr: rmt_result.as_ref().and_then(|r| {
            if r.filtered_corr.is_empty() {
                None
            } else {
                Some(r.filtered_corr.clone())
            }
        }),
        te_scores: None, // TE computed separately per-pair on demand
        last_updated: bar_idx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_neutral() {
        let state = CharacterizationState::default();
        assert_eq!(state.hurst, 0.5);
        assert_eq!(state.hmm_state, 0);
        assert!(state.cleaned_corr.is_none());
        assert!(state.te_scores.is_none());
        assert_eq!(state.last_updated, 0);
    }

    #[test]
    fn characterize_skips_when_interval_not_elapsed() {
        let prev = CharacterizationState {
            hurst: 0.6,
            hmm_state: 2,
            cleaned_corr: None,
            te_scores: None,
            last_updated: 50,
        };

        let close = vec![100.0; 600];
        let returns = vec![0.001; 600];

        let result = characterize(&close, &returns, None, 80, 100, 500, &prev);
        // Only 30 bars elapsed (80 - 50 < 100), so should return prev.
        assert_eq!(result.hurst, 0.6);
        assert_eq!(result.hmm_state, 2);
        assert_eq!(result.last_updated, 50);
    }

    #[test]
    fn characterize_recomputes_when_interval_elapsed() {
        let prev = CharacterizationState::default();
        let close: Vec<f32> = (0..600).map(|i| 100.0 + i as f32 * 0.05).collect();
        let returns: Vec<f32> = (0..600).map(|_| 0.001).collect();

        let result = characterize(&close, &returns, None, 200, 100, 500, &prev);
        assert_eq!(result.last_updated, 200);
        // Hurst should be recomputed (trending data).
        assert!(result.hurst > 0.0);
    }
}
