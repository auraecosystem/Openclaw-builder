# Kalman Trend Filter
2×2 constant-velocity Kalman filter. State = [level, trend]. Zero-lag trend estimation with optimal noise filtering. Outputs both filtered price (level) and trend velocity per bar.

## Use Cases
- Zero-lag trend estimation: Kalman-optimal smoother without the lag penalty of EMA/SMA
- Trend velocity extraction: the `trend` output captures rate-of-change without differencing noise
- Noise filtering: denoised price suitable for downstream pattern detection

## Algorithm

State transition (`F = [[1,1],[0,1]]`): `level_pred = level + trend`, `trend_pred = trend`.

Observation model (`H = [1, 0]`): only level is observed.

Each bar:
1. **Predict**: propagate state and covariance through `F`, add process noise `Q = q·I`
2. **Update**: compute innovation `z - level_pred`, Kalman gain `K = P_pred·H' / (P_pred[0,0] + R)`, correct state and covariance

Initial covariance is set to `1e6·I` (high uncertainty) and converges within a few bars.

## Parameters

| Parameter | Default | Description |
|---|---|---|
| `period` | — | Bars before marking as initialized (minimum 1) |
| `process_noise` | 1e-4 | Process noise variance Q; lower = smoother but slower to adapt |
| `measurement_noise` | 1e-2 | Observation noise variance R; higher = smoother output |

Lower `process_noise` / higher `measurement_noise` → smoother level, slower response. Inverse for faster tracking.

## Output
- `value: f64` — filtered price level (same as `level`)
- `level: f64` — filtered price level
- `trend: f64` — estimated rate of change per bar

## Status
Implemented. Engine: `kalman.rs`. NT: `KalmanTrendFilter`.
