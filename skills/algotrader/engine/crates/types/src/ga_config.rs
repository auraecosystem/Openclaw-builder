//! Runtime-configurable constants for the genetic algorithm optimizer.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GaConfig {
    /// Initial mutation sigma (std dev as fraction of gene range).
    #[serde(default = "default_base_sigma")]
    pub base_sigma: f64,

    /// Generations without improvement before sigma escalation.
    #[serde(default = "default_stale_limit")]
    pub stale_limit: usize,

    /// Generations without improvement before cataclysm reset.
    #[serde(default = "default_cataclysm_limit")]
    pub cataclysm_limit: usize,

    /// Additive sigma step on stale detection.
    #[serde(default = "default_stale_sigma_step")]
    pub stale_sigma_step: f64,

    /// Multiplicative sigma factor during cataclysm.
    #[serde(default = "default_cataclysm_sigma_mult")]
    pub cataclysm_sigma_mult: f64,

    /// Fraction of population replaced with random individuals during cataclysm.
    #[serde(default = "default_cataclysm_random_frac")]
    pub cataclysm_random_frac: f64,

    /// Fraction of population replaced with random individuals on mild injection.
    #[serde(default = "default_mild_injection_frac")]
    pub mild_injection_frac: f64,

    /// Tournament selection size.
    #[serde(default = "default_tournament_k")]
    pub tournament_k: usize,

    /// Per-gene probability of mutation.
    #[serde(default = "default_mutation_rate")]
    pub mutation_rate: f64,

    /// Probability of crossover between two parents.
    #[serde(default = "default_crossover_prob")]
    pub crossover_prob: f64,
}

fn default_base_sigma() -> f64 { 0.15 }
fn default_stale_limit() -> usize { 5 }
fn default_cataclysm_limit() -> usize { 12 }
fn default_stale_sigma_step() -> f64 { 0.3 }
fn default_cataclysm_sigma_mult() -> f64 { 2.0 }
fn default_cataclysm_random_frac() -> f64 { 0.5 }
fn default_mild_injection_frac() -> f64 { 0.3 }
fn default_tournament_k() -> usize { 3 }
fn default_mutation_rate() -> f64 { 0.3 }
fn default_crossover_prob() -> f64 { 0.5 }

impl Default for GaConfig {
    fn default() -> Self {
        Self {
            base_sigma: default_base_sigma(),
            stale_limit: default_stale_limit(),
            cataclysm_limit: default_cataclysm_limit(),
            stale_sigma_step: default_stale_sigma_step(),
            cataclysm_sigma_mult: default_cataclysm_sigma_mult(),
            cataclysm_random_frac: default_cataclysm_random_frac(),
            mild_injection_frac: default_mild_injection_frac(),
            tournament_k: default_tournament_k(),
            mutation_rate: default_mutation_rate(),
            crossover_prob: default_crossover_prob(),
        }
    }
}
