//! 0.2 HMM Viterbi (manual 3-state)
//!
//! Pre-trained 3-state Gaussian HMM decoded via Viterbi on trailing returns.
//! States: 0=low-vol, 1=high-vol, 2=trending.

/// Pre-trained HMM parameters.
///
/// Transition probabilities and Gaussian emission parameters are stored in
/// log-space where possible to avoid underflow during Viterbi.
#[derive(Clone, Debug)]
pub struct HmmModel {
    /// Number of hidden states (always 3 for this model).
    pub n_states: usize,
    /// Log transition probabilities: `log_trans[i][j]` = ln P(state j | state i).
    pub log_trans: [[f32; 3]; 3],
    /// Gaussian emission means per state (one per state).
    pub emission_mean: [f32; 3],
    /// Gaussian emission variances per state.
    pub emission_var: [f32; 3],
    /// Log initial state probabilities.
    pub log_init: [f32; 3],
}

impl Default for HmmModel {
    /// Reasonable defaults calibrated to daily crypto returns:
    ///
    /// - State 0 (low-vol): mean ~0, small variance -- quiet consolidation.
    /// - State 1 (high-vol): mean ~0, large variance -- choppy/volatile.
    /// - State 2 (trending): positive mean, moderate variance -- momentum.
    fn default() -> Self {
        // Transition matrix (row = from, col = to). Probabilities sum to 1 per
        // row, stored as ln(p).
        //
        //        to: 0        1        2
        // from 0: [0.90,    0.05,    0.05]
        // from 1: [0.10,    0.80,    0.10]
        // from 2: [0.05,    0.10,    0.85]
        let log_trans = [
            [ln(0.90), ln(0.05), ln(0.05)],
            [ln(0.10), ln(0.80), ln(0.10)],
            [ln(0.05), ln(0.10), ln(0.85)],
        ];

        Self {
            n_states: 3,
            log_trans,
            emission_mean: [0.0, 0.0, 0.002],
            emission_var: [0.0001, 0.001, 0.0004],
            log_init: [ln(0.4), ln(0.3), ln(0.3)],
        }
    }
}

/// Run the Viterbi algorithm on `returns` using the given HMM model.
///
/// Returns the most-likely current state (0, 1, or 2). On degenerate input
/// (empty slice, NaN values, zero variance) returns 0.
#[allow(clippy::needless_range_loop)]
pub fn compute_hmm_state(returns: &[f32], model: &HmmModel) -> u8 {
    if returns.is_empty() {
        return 0;
    }

    // Validate variances are positive.
    for &v in &model.emission_var {
        if v <= 0.0 || v.is_nan() {
            return 0;
        }
    }

    let n = returns.len();
    let ns = model.n_states.min(3);

    // Viterbi: keep only the previous column of log-probabilities (no full
    // trellis needed since we only want the final state).
    let mut prev = [0.0_f32; 3];
    let first_obs = returns[0];
    if first_obs.is_nan() {
        return 0;
    }
    for s in 0..ns {
        prev[s] = model.log_init[s] + gaussian_log_likelihood(first_obs, model.emission_mean[s], model.emission_var[s]);
    }

    let mut curr = [0.0_f32; 3];

    for t in 1..n {
        let obs = returns[t];
        if obs.is_nan() {
            // Skip NaN observations: carry forward previous state probs.
            continue;
        }

        for j in 0..ns {
            let emit = gaussian_log_likelihood(obs, model.emission_mean[j], model.emission_var[j]);
            // max over predecessor states
            let mut best = f32::NEG_INFINITY;
            for i in 0..ns {
                let score = prev[i] + model.log_trans[i][j];
                if score > best {
                    best = score;
                }
            }
            curr[j] = best + emit;
        }

        prev = curr;
    }

    // Return state with highest log-probability.
    argmax3(&prev) as u8
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Log-likelihood of observing `x` under N(mu, var).
///
/// ln N(x | mu, var) = -0.5 * ln(2*pi*var) - (x - mu)^2 / (2*var)
#[inline]
fn gaussian_log_likelihood(x: f32, mu: f32, var: f32) -> f32 {
    const LN_2PI: f32 = 1.837_877; // ln(2*pi)
    let diff = x - mu;
    -0.5 * (LN_2PI + var.ln() + diff * diff / var)
}

/// Argmax of a 3-element array. Ties broken by lowest index.
#[inline]
fn argmax3(v: &[f32; 3]) -> usize {
    let mut best = 0;
    if v[1] > v[best] {
        best = 1;
    }
    if v[2] > v[best] {
        best = 2;
    }
    best
}

/// Compile-time-evaluable `ln` is not available, so just use the runtime
/// version. These are called once at model construction.
#[inline]
fn ln(x: f32) -> f32 {
    x.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_returns_default() {
        let model = HmmModel::default();
        assert_eq!(compute_hmm_state(&[], &model), 0);
    }

    #[test]
    fn single_observation() {
        let model = HmmModel::default();
        let state = compute_hmm_state(&[0.0], &model);
        assert!(state < 3);
    }

    #[test]
    fn low_vol_returns_detect_state_0() {
        let model = HmmModel::default();
        // Tiny returns => low-vol state expected.
        let returns: Vec<f32> = (0..200).map(|_| 0.00001).collect();
        let state = compute_hmm_state(&returns, &model);
        assert_eq!(state, 0, "expected low-vol state for tiny returns");
    }

    #[test]
    fn different_regimes_produce_different_states() {
        let model = HmmModel::default();
        // Tiny near-zero returns (low-vol)
        let quiet: Vec<f32> = (0..200).map(|_| 0.00001).collect();
        let s_quiet = compute_hmm_state(&quiet, &model);

        // Large volatile returns
        let volatile: Vec<f32> = (0..200).map(|i| if i % 2 == 0 { 0.05 } else { -0.05 }).collect();
        let s_vol = compute_hmm_state(&volatile, &model);

        // The two regimes should decode to different states
        assert_ne!(s_quiet, s_vol, "quiet and volatile should produce different states");
        assert!(s_quiet < 3 && s_vol < 3);
    }

    #[test]
    fn nan_handling() {
        let model = HmmModel::default();
        // Single NaN → first observation is NaN → return default state 0
        assert_eq!(compute_hmm_state(&[f32::NAN], &model), 0);
        // NaN in middle is skipped; small returns stay in state 0
        let state = compute_hmm_state(&[0.001, f32::NAN, 0.001], &model);
        assert!(state < 3, "NaN-skipped sequence should produce valid state");
    }
}
