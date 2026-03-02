//! Runtime-configurable constants for the genetic algorithm optimizer.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct GaConfig {
    /// Initial mutation sigma (std dev as fraction of gene range).
    pub base_sigma: f64,

    /// Generations without improvement before sigma escalation.
    pub stale_limit: usize,

    /// Generations without improvement before cataclysm reset.
    pub cataclysm_limit: usize,

    /// Additive sigma step on stale detection.
    pub stale_sigma_step: f64,

    /// Multiplicative sigma factor during cataclysm.
    pub cataclysm_sigma_mult: f64,

    /// Fraction of population replaced with random individuals during cataclysm.
    pub cataclysm_random_frac: f64,

    /// Fraction of population replaced with random individuals on mild injection.
    pub mild_injection_frac: f64,

    /// Tournament selection size.
    pub tournament_k: usize,

    /// Per-gene probability of mutation.
    pub mutation_rate: f64,

    /// Probability of crossover between two parents.
    pub crossover_prob: f64,
}

impl Default for GaConfig {
    fn default() -> Self {
        Self {
            base_sigma: 0.15,
            stale_limit: 5,
            cataclysm_limit: 12,
            stale_sigma_step: 0.3,
            cataclysm_sigma_mult: 2.0,
            cataclysm_random_frac: 0.5,
            mild_injection_frac: 0.3,
            tournament_k: 3,
            mutation_rate: 0.3,
            crossover_prob: 0.5,
        }
    }
}
