//! Evolutionary parameter optimization.
//!
//! Two optimizers: CMA-ES (self-adaptive, best for continuous ~44D) and a
//! simple GA (port of scripts/evolve.py). Both use rayon for parallel
//! fitness evaluation.

pub mod cmaes;
pub mod fitness;
pub mod ga;
pub mod genome;

// Re-export config types from engine-types so existing paths still resolve.
pub use engine_types::cmaes_config;
pub use engine_types::fitness_config;
pub use engine_types::ga_config;

use rand::rngs::StdRng;
use rand::SeedableRng;
use rayon::prelude::*;
use serde::Serialize;

use engine_data::DataStore;
use engine_types::Params;

use self::fitness::{evaluate, FitnessMetric};
use self::genome::GenomeSpec;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Which optimizer to use.
#[derive(Clone, Debug)]
pub enum Algorithm {
    CmaEs,
    Ga,
}

impl Algorithm {
    pub fn parse(s: &str) -> Self {
        match s {
            "ga" | "GA" => Algorithm::Ga,
            _ => Algorithm::CmaEs,
        }
    }
}

/// Configuration for an evolution run.
#[derive(Clone, Debug)]
pub struct EvolutionConfig {
    pub algorithm: Algorithm,
    pub generations: usize,
    pub pop_size: usize,
    pub fitness_metric: FitnessMetric,
    pub initial_sigma: f64,
    pub evolve_signals: bool,
    pub crypto: bool,
    pub seed: Option<u64>,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::CmaEs,
            generations: 50,
            pop_size: 0, // 0 = use CMA-ES default lambda
            fitness_metric: FitnessMetric::Composite,
            initial_sigma: 0.3,
            evolve_signals: false,
            crypto: false,
            seed: None,
        }
    }
}

/// Per-generation log entry.
#[derive(Clone, Debug, Serialize)]
pub struct GenerationLog {
    pub generation: usize,
    pub best_fitness: f64,
    pub mean_fitness: f64,
    pub diversity: f64,
    pub sigma: f64,
    pub total_trades: usize,
    pub best_return: f64,
}

/// Final result of an evolution run.
#[derive(Clone, Debug, Serialize)]
pub struct EvolutionResult {
    pub best_params: serde_json::Value,
    pub best_report: serde_json::Value,
    pub best_fitness: f64,
    pub generations_run: usize,
    pub history: Vec<GenerationLog>,
}

/// Result of walk-forward evolution.
#[derive(Clone, Debug, Serialize)]
pub struct WalkForwardEvolutionResult {
    pub folds: Vec<WfFold>,
    pub mean_train_fitness: f64,
    pub mean_test_fitness: f64,
}

/// A single train/test fold within walk-forward evolution.
#[derive(Clone, Debug, Serialize)]
pub struct WfFold {
    pub fold_idx: usize,
    pub train_fitness: f64,
    pub test_fitness: f64,
    pub test_report: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Main evolution entry point
// ---------------------------------------------------------------------------

/// Run evolutionary optimization on the given data store.
///
/// Dispatches to CMA-ES or GA depending on `config.algorithm`. Both
/// optimizers evaluate fitness in parallel via rayon. Returns the best
/// parameters found along with generation-by-generation history.
pub fn evolve(store: &DataStore, base: &Params, config: &EvolutionConfig) -> EvolutionResult {
    let spec = if config.evolve_signals {
        GenomeSpec::combined_spec(config.crypto)
    } else {
        GenomeSpec::params_spec(config.crypto)
    };

    let mut rng = match config.seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };

    let mut history = Vec::new();
    let mut best_fitness = f64::NEG_INFINITY;
    let mut best_params_json = serde_json::Value::Null;
    let mut best_report_json = serde_json::Value::Null;

    match config.algorithm {
        Algorithm::CmaEs => {
            run_cmaes(
                store,
                base,
                config,
                &spec,
                &mut rng,
                &mut history,
                &mut best_fitness,
                &mut best_params_json,
                &mut best_report_json,
            );
        }
        Algorithm::Ga => {
            run_ga(
                store,
                base,
                config,
                &spec,
                &mut rng,
                &mut history,
                &mut best_fitness,
                &mut best_params_json,
                &mut best_report_json,
            );
        }
    }

    EvolutionResult {
        best_params: best_params_json,
        best_report: best_report_json,
        best_fitness,
        generations_run: config.generations,
        history,
    }
}

// ---------------------------------------------------------------------------
// Walk-forward evolution
// ---------------------------------------------------------------------------

/// Walk-forward evolution with expanding training window.
///
/// Splits data into `n_folds` sequential test blocks. For each fold, runs
/// a full evolution on data prior to the test block, then evaluates the
/// best parameters out-of-sample. This mirrors `walk_forward.rs` but uses
/// evolutionary search instead of grid sweep.
pub fn evolve_walk_forward(
    store: &DataStore,
    base: &Params,
    config: &EvolutionConfig,
    n_folds: usize,
) -> WalkForwardEvolutionResult {
    let total_rows = store.axes.n_rows;
    let fold_size = total_rows / n_folds;
    let mut folds = Vec::new();

    for fold_idx in 0..n_folds {
        let test_start = fold_size * fold_idx + (total_rows - fold_size * n_folds);
        let test_end = test_start + fold_size;
        let train_end = test_start;

        // Need enough training data for meaningful optimization.
        if train_end < 100 {
            continue;
        }

        eprintln!(
            "Fold {}/{}: train [0..{}), test [{}..{})",
            fold_idx + 1,
            n_folds,
            train_end,
            test_start,
            test_end
        );

        // Run evolution on the training window.
        let result = evolve(store, base, config);

        // Evaluate the best params on the test window.
        let best_params: Params =
            serde_json::from_value(result.best_params.clone()).unwrap_or_else(|_| base.clone());
        let (test_report, test_fitness) = evaluate(store, &best_params, &config.fitness_metric, &best_params.fitness);

        folds.push(WfFold {
            fold_idx,
            train_fitness: result.best_fitness,
            test_fitness,
            test_report,
        });
    }

    let n = folds.len().max(1) as f64;
    WalkForwardEvolutionResult {
        mean_train_fitness: folds.iter().map(|f| f.train_fitness).sum::<f64>() / n,
        mean_test_fitness: folds.iter().map(|f| f.test_fitness).sum::<f64>() / n,
        folds,
    }
}

// ---------------------------------------------------------------------------
// CMA-ES dispatch (private)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn run_cmaes(
    store: &DataStore,
    base: &Params,
    config: &EvolutionConfig,
    spec: &GenomeSpec,
    rng: &mut StdRng,
    history: &mut Vec<GenerationLog>,
    best_fitness: &mut f64,
    best_params_json: &mut serde_json::Value,
    best_report_json: &mut serde_json::Value,
) {
    let initial_mean = spec.encode(base);
    let mut optimizer = cmaes::CmaEs::new(spec.dim(), initial_mean, config.initial_sigma, base.cmaes.clone());

    for gen in 0..config.generations {
        let population = optimizer.ask(rng);

        // Clamp sampled genomes to gene bounds.
        let clamped: Vec<Vec<f64>> = population
            .into_iter()
            .map(|mut g| {
                spec.clamp(&mut g);
                g
            })
            .collect();

        // Evaluate fitness in parallel.
        let results: Vec<(serde_json::Value, f64)> = clamped
            .par_iter()
            .map(|genome| {
                let params = spec.decode(genome, base);
                evaluate(store, &params, &config.fitness_metric, &base.fitness)
            })
            .collect();

        // Pair genomes with fitness, sorted best-first.
        let mut scored: Vec<(Vec<f64>, f64)> = clamped
            .into_iter()
            .zip(results.iter().map(|(_, f)| *f))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let gen_best_report = results
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(r, _)| r.clone())
            .unwrap_or_default();

        record_generation(
            gen,
            config,
            spec,
            base,
            &scored,
            &gen_best_report,
            &optimizer.sigma(),
            history,
            best_fitness,
            best_params_json,
            best_report_json,
        );

        optimizer.tell(&scored);
    }
}

// ---------------------------------------------------------------------------
// GA dispatch (private)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn run_ga(
    store: &DataStore,
    base: &Params,
    config: &EvolutionConfig,
    spec: &GenomeSpec,
    rng: &mut StdRng,
    history: &mut Vec<GenerationLog>,
    best_fitness: &mut f64,
    best_params_json: &mut serde_json::Value,
    best_report_json: &mut serde_json::Value,
) {
    let pop_size = if config.pop_size > 0 {
        config.pop_size
    } else {
        100
    };
    let mut optimizer = ga::Ga::new(spec.dim(), pop_size, 0.25, base.ga.clone());

    let bounds: Vec<(f64, f64, bool, bool)> = spec
        .bounds
        .iter()
        .map(|b| (b.min, b.max, b.is_integer, b.is_boolean))
        .collect();

    // Seed population randomly within bounds.
    let mut population: Vec<Vec<f64>> = (0..pop_size).map(|_| spec.random(rng)).collect();

    for gen in 0..config.generations {
        // Evaluate fitness in parallel.
        let results: Vec<(serde_json::Value, f64)> = population
            .par_iter()
            .map(|genome| {
                let params = spec.decode(genome, base);
                evaluate(store, &params, &config.fitness_metric, &base.fitness)
            })
            .collect();

        // Pair genomes with fitness, sorted best-first.
        let mut scored: Vec<(Vec<f64>, f64)> = population
            .into_iter()
            .zip(results.iter().map(|(_, f)| *f))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let gen_best_report = results
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(r, _)| r.clone())
            .unwrap_or_default();

        record_generation(
            gen,
            config,
            spec,
            base,
            &scored,
            &gen_best_report,
            &optimizer.sigma(),
            history,
            best_fitness,
            best_params_json,
            best_report_json,
        );

        // Evolve next generation.
        population = optimizer.evolve_step(&scored, &bounds, rng);
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Record a generation's statistics and update the global best if improved.
#[allow(clippy::too_many_arguments)]
fn record_generation(
    gen: usize,
    config: &EvolutionConfig,
    spec: &GenomeSpec,
    base: &Params,
    scored: &[(Vec<f64>, f64)],
    gen_best_report: &serde_json::Value,
    sigma: &f64,
    history: &mut Vec<GenerationLog>,
    best_fitness: &mut f64,
    best_params_json: &mut serde_json::Value,
    best_report_json: &mut serde_json::Value,
) {
    let gen_best = scored[0].1;
    let gen_mean = scored.iter().map(|(_, f)| f).sum::<f64>() / scored.len() as f64;

    let improved = gen_best > *best_fitness;
    if improved {
        *best_fitness = gen_best;
        *best_params_json =
            serde_json::to_value(spec.decode(&scored[0].0, base)).unwrap_or_default();
        *best_report_json = gen_best_report.clone();
    }

    let trades = gen_best_report["total_trades"]
        .as_u64()
        .unwrap_or(0) as usize;
    let ret = gen_best_report["total_return"].as_f64().unwrap_or(0.0);

    history.push(GenerationLog {
        generation: gen,
        best_fitness: gen_best,
        mean_fitness: gen_mean,
        diversity: population_diversity(scored),
        sigma: *sigma,
        total_trades: trades,
        best_return: ret,
    });

    eprintln!(
        "  Gen {:3}/{}: best={:.4} mean={:.4} trades={} ret={:.4} \u{03c3}={:.4}{}",
        gen + 1,
        config.generations,
        gen_best,
        gen_mean,
        trades,
        ret,
        sigma,
        if improved { " *" } else { "" }
    );
}

/// Mean normalized standard deviation across all genes.
///
/// Higher values indicate more spread in the population (exploration);
/// near-zero means the population has converged.
fn population_diversity(scored: &[(Vec<f64>, f64)]) -> f64 {
    if scored.len() < 2 || scored[0].0.is_empty() {
        return 0.0;
    }
    let dim = scored[0].0.len();
    let n = scored.len() as f64;
    let mut total_div = 0.0;

    for d in 0..dim {
        let vals: Vec<f64> = scored.iter().map(|(g, _)| g[d]).collect();
        let mean = vals.iter().sum::<f64>() / n;
        let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        let range = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - vals.iter().cloned().fold(f64::INFINITY, f64::min);
        if range > 0.0 {
            total_div += var.sqrt() / range;
        }
    }

    total_div / dim as f64
}
