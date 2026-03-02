//! 2.3 Kronos foundation model forecast -- cold-path pyo3 callout stub.
//!
//! The real implementation will call through pyo3 to the Kronos or Chronos-2
//! foundation model for a probabilistic forecast.  Until the pyo3 bridge is
//! wired, this returns neutral values that have no influence on the
//! investigation decision.

/// Kronos foundation model forecast (stub -- requires pyo3 bridge).
///
/// Returns `(direction, magnitude, confidence)` where:
///   - `direction`:  predicted direction (-1 down, 0 flat, +1 up)
///   - `magnitude`:  predicted move size (percentage)
///   - `confidence`: model confidence (0 = uncertain, 1 = certain)
///
/// Currently returns neutral values: direction=0 (flat), magnitude=0,
/// confidence=0.  This ensures the investigation layer treats the forecast
/// as uninformative rather than contradictory.
pub fn kronos_forecast(_close: &[f32]) -> (f32, f32, f32) {
    // Neutral: direction=0 (flat), magnitude=0, confidence=0
    (0.0, 0.0, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_neutral() {
        let (dir, mag, conf) = kronos_forecast(&[1.0, 2.0, 3.0]);
        assert_eq!(dir, 0.0);
        assert_eq!(mag, 0.0);
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn stub_handles_empty_input() {
        let (dir, mag, conf) = kronos_forecast(&[]);
        assert_eq!(dir, 0.0);
        assert_eq!(mag, 0.0);
        assert_eq!(conf, 0.0);
    }
}
