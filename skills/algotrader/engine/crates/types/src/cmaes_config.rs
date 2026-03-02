//! Runtime-configurable constants for the CMA-ES optimizer.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CmaEsConfig {
    /// Upper clamp for the step-size sigma (prevents explosion).
    pub sigma_clamp_hi: f64,

    /// Divisor for eigendecomposition interval (interval = dim / this).
    pub decomp_interval_div: usize,
}

impl Default for CmaEsConfig {
    fn default() -> Self {
        Self {
            sigma_clamp_hi: 1e2,
            decomp_interval_div: 10,
        }
    }
}
