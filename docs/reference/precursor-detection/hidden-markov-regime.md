# Hidden Markov Regime Detection (HMM Viterbi)
3-state Gaussian HMM decoded via Viterbi. States: 0 = low-volatility consolidation, 1 = high-volatility choppy, 2 = trending. Pre-trained emission parameters; no in-flight EM re-estimation.

## Use Cases
- Regime labeling: assign each bar a probabilistic regime index
- Strategy gating: only enter breakout trades when regime = 2 (trending)
- Volatility context: high-vol state flags elevated risk for position sizing

## Algorithm

1. Compute log-returns from the `period`-bar rolling price window
2. Run the Viterbi algorithm in log-space: `v_t(j) = max_i(v_{t-1}(i) + log_trans[i][j]) + log_emit(obs_t, j)`
3. Take the argmax of the final Viterbi vector as the most-likely terminal state
4. Normalize state index to [0, 1]: `value = state / (n_states - 1)`

Initialization: uniform prior across states. Transition matrix: diagonal = `transition_persistence`, off-diagonal probability split equally across remaining states.

## Parameters

| Parameter | Default | Description |
|---|---|---|
| `period` | — | Rolling window of close prices |
| `n_states` | 3 | Number of HMM states (capped at 3 internally) |
| `transition_persistence` | 0.85 | Probability of staying in the same state (diagonal of transition matrix) |
| `emission_means` | [0.0, 0.0, 0.002] | Per-state return means for states [low-vol, high-vol, trending] |
| `emission_vars` | [0.0001, 0.001, 0.0004] | Per-state return variances |

## Output
- `value: f64` — Viterbi state normalized to [0, 1] (0 = low-vol, 0.5 = high-vol, 1 = trending for 3-state)

## Status
Implemented. Engine: `hmm.rs`. NT: `HiddenMarkovRegime`.
