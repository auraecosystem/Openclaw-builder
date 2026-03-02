//! 1.3 Kalman Filter -- 2x2 constant-velocity model for zero-lag trend estimation.
//!
//! State vector: [level, trend].  The transition model assumes the trend
//! persists between bars (constant velocity).  The observation model sees
//! only the level (noisy close price).
//!
//! All matrix operations are inlined 2x2 -- no linear algebra crate needed.

/// Update Kalman filter state with a new close price observation.
///
/// # Arguments
/// * `x` -- state vector `[level, trend]`, mutated in place.
/// * `p` -- covariance matrix (2x2, row-major flat), mutated in place.
/// * `obs` -- observed close price for this bar.
/// * `q` -- process noise scalar (added to diagonal of P during predict).
/// * `r` -- measurement noise scalar.
///
/// # Returns
/// `(level, trend)` after the update step.
pub fn kalman_update(
    x: &mut [f32; 2],
    p: &mut [f32; 4],
    obs: f32,
    q: f32,
    r: f32,
) -> (f32, f32) {
    // -- Predict --
    // F = [[1, 1], [0, 1]]
    // x_pred = F * x  =>  level += trend,  trend unchanged.
    let x0 = x[0] + x[1];
    let x1 = x[1];

    // P_pred = F * P * F' + Q * I
    // F * P (row-major: P = [p00, p01, p10, p11])
    //   fp00 = p00 + p10,  fp01 = p01 + p11
    //   fp10 = p10,        fp11 = p11
    let fp00 = p[0] + p[2];
    let fp01 = p[1] + p[3];
    let fp10 = p[2];
    let fp11 = p[3];
    // (F*P) * F'  where F' = [[1,0],[1,1]]
    let pp00 = fp00 + fp01 + q;
    let pp01 = fp01;
    let pp10 = fp10 + fp11;
    let pp11 = fp11 + q;

    // -- Update --
    // H = [1, 0]
    // S = H * P_pred * H' + R  =  pp00 + r
    let s = pp00 + r;
    if s.abs() < 1e-30 {
        // Degenerate -- skip update to avoid division by zero.
        x[0] = x0;
        x[1] = x1;
        return (x0, x1);
    }
    let s_inv = 1.0 / s;

    // K = P_pred * H' * S^-1  =  [pp00, pp10]' * s_inv
    let k0 = pp00 * s_inv;
    let k1 = pp10 * s_inv;

    // Innovation
    let y = obs - x0;

    // x = x_pred + K * y
    x[0] = x0 + k0 * y;
    x[1] = x1 + k1 * y;

    // P = (I - K*H) * P_pred
    // K*H = [[k0, 0], [k1, 0]]
    p[0] = pp00 - k0 * pp00;
    p[1] = pp01 - k0 * pp01;
    p[2] = pp10 - k1 * pp00;
    p[3] = pp11 - k1 * pp01;

    (x[0], x[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_constant_trend() {
        let mut x = [0.0_f32; 2];
        let mut p = [1.0, 0.0, 0.0, 1.0];
        // Feed a linearly increasing series: 10, 11, 12, ...
        for i in 0..50 {
            let obs = 10.0 + i as f32;
            kalman_update(&mut x, &mut p, obs, 0.001, 1.0);
        }
        // Level should be near 59, trend near 1.
        assert!((x[0] - 59.0).abs() < 1.0, "level = {}", x[0]);
        assert!((x[1] - 1.0).abs() < 0.2, "trend = {}", x[1]);
    }
}
