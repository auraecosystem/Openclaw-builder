//! Runtime-configurable constants for the CMA-ES optimizer.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CmaEsConfig {
    /// Upper clamp for the step-size sigma (prevents explosion).
    #[serde(default = "default_sigma_clamp_hi")]
    pub sigma_clamp_hi: f64,

    /// Divisor for eigendecomposition interval (interval = dim / this).
    #[serde(default = "default_decomp_interval_div")]
    pub decomp_interval_div: usize,
}

fn default_sigma_clamp_hi() -> f64 { 1e2 }
fn default_decomp_interval_div() -> usize { 10 }

impl Default for CmaEsConfig {
    fn default() -> Self {
        Self {
            sigma_clamp_hi: default_sigma_clamp_hi(),
            decomp_interval_div: default_decomp_interval_div(),
        }
    }
}
