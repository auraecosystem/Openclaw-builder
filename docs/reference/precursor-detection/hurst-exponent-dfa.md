# Hurst Exponent (DFA)
Detrended Fluctuation Analysis. H > 0.55 = trending (persistent), H < 0.45 = mean-reverting (anti-persistent), H ≈ 0.5 = random walk. Measures long-range dependence in the price series.

## Use Cases
- Regime classification: trending vs. mean-reverting vs. random-walk market state
- Strategy selection: momentum strategies suit H > 0.55; mean-reversion suits H < 0.45
- Detecting persistence shifts: rising H signals onset of a trending regime

## Limitations
- Requires at least 16 samples for a reliable estimate; unreliable below this threshold
- Log-log regression slope noisy on short windows — use `period ≥ 32`
- Measures a single global H over the window; does not capture local regime changes

## Algorithm

1. Build the cumulative-sum (profile) of mean-subtracted prices
2. Generate geometrically spaced box sizes from 4 to `period/4` (step ≈ ×1.5)
3. For each box size: divide the profile into non-overlapping boxes, fit a linear trend in each box, compute the RMS of residuals
4. Regress log(RMS) on log(box_size) via ordinary least squares
5. The slope of that regression is H; clamp to [0, 1]

Returns 0.5 (random walk default) if fewer than 2 box sizes are available.

## Parameters

| Parameter | Description |
|---|---|
| `period` | Rolling window length; should be ≥ 32 for reliable estimates |

## Output
- `value: f64` — Hurst exponent ∈ [0, 1]

## Status
Implemented. Engine: `hurst.rs`. NT: `HurstExponent`.
