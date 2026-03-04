# Bayesian Online Changepoint Detection (BOCPD)
Adams & MacKay (2007). Maintains a posterior over run lengths — probability distribution over "how long since the last changepoint." Naturally yields P(changepoint at this bar). Uses Normal-Inverse-Gamma conjugate prior for online Bayesian updating.

## Use Cases
- Online structural break detection: real-time probability that the current bar marks a regime shift
- Regime change probability: continuous [0,1] signal rather than a binary flag
- Run-length estimation: the run-length posterior also reveals how long the current regime has persisted

## Algorithm

1. On each bar, compute log-return `r = close / prev_close - 1`
2. For each run-length hypothesis, evaluate the Student-t predictive likelihood under the NIG posterior
3. Grow existing hypotheses (survival probability = `1 - hazard`) and collapse all mass to run-length 0 (changepoint probability = `hazard · Σ(posterior × likelihood)`)
4. Normalize the full run-length distribution; prune hypotheses older than `period`
5. Output = posterior weight at run-length 0 = P(changepoint)

Conjugate prior: NIG with parameters (`prior_mu`, `prior_kappa`, `prior_alpha`, `prior_beta`). Sufficient statistics updated incrementally per hypothesis.

## Parameters

| Parameter | Default | Description |
|---|---|---|
| `period` | — | Maximum run length tracked; older hypotheses pruned to bound memory |
| `hazard_lambda` | 50.0 | Expected bars between changepoints; hazard rate = 1/hazard_lambda |
| `prior_mu` | 0.0 | NIG prior mean for returns |
| `prior_kappa` | 1.0 | NIG prior precision weight |
| `prior_alpha` | 1.0 | NIG prior shape |
| `prior_beta` | 0.01 | NIG prior scale |

## Output
- `value: f64` — P(changepoint at current bar) ∈ [0, 1]

## Status
Implemented. Engine: `bocpd.rs`. NT: `BayesianChangepoint`.
