# Discrete-Time Survival / Hazard Model

Nonparametric estimation of time-to-event distributions. Tracks historical threshold-crossing
events to estimate survival curves and hazard rates. Answers: "given the current state, how likely
is a threshold crossing in the next N bars?"

## Use Cases

- Time-to-termination probability for open positions
- Historical event-rate estimation for position sizing
- Complement to first-passage: nonparametric vs parametric
- Censoring-aware: handles events that haven't happened yet

## Limitations

- Requires historical event data (labeled threshold crossings)
- Hazard estimates noisy with few events
- Censoring rules must be carefully defined
- Drift invalidates stationarity assumption unless re-estimated

## Algorithm

**Kaplan-Meier:** `Ŝ(t) = Π_{t_i≤t} (1 - d_i/n_i)`

**Nelson-Aalen:** `Ĥ(t) = Σ_{t_i≤t} d_i/n_i`, S(t) = exp(-Ĥ(t))

**Discrete hazard:** `h_t = P(T=t | T≥t) = f(t)/S(t-1)`

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| History window | `period` | 500 | 100–2000 | Event history |
| Event threshold | `event_threshold` | 2.0×σ | 1–5×σ | "Failure" definition |
| Censoring window | `censoring_window` | 50 | 10–252 | Max observation |
| Smoothing bandwidth | `smoothing_bandwidth` | n^(-1/5) | — | Kernel for hazard |
| Method | `method` | nelson_aalen | km/nelson_aalen | Estimator |

## Complexity

O(n_events) per update.

## Output

- `value: f64` — S(t) survival probability
- `cumulative_hazard: f64`, `hazard_rate: f64`, `median_survival: f64`

## References

Kaplan & Meier (1958), Nelson (1972), Aalen (1978)

## Status

To implement. Zero deps. Tier 2.
