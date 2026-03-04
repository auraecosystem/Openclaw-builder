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

use self::fitness::{evaluate, evaluate_dynamic, FitnessMetric};
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
    pub evolve_patterns: bool,
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
            evolve_patterns: false,
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
///
/// For dynamic JSON strategies (loaded from `strategies/{name}.json`), the
/// genome spec is built from the JSON's `params` block and each evaluation
/// re-creates the DynamicSetup with overridden params. For hardcoded
/// strategies, the legacy `Params` struct genome spec is used.
pub fn evolve(store: &DataStore, base: &Params, config: &EvolutionConfig) -> EvolutionResult {
    // Check if this is a dynamic JSON strategy with its own evolvable params.
    let pipeline_params = load_pipeline_params(&base.setup);

    let spec = if pipeline_params.is_some() {
        let pp = pipeline_params.as_ref().unwrap();
        eprintln!(
            "  Dynamic strategy '{}': evolving {} pipeline params",
            base.setup,
            pp.len()
        );
        GenomeSpec::from_pipeline_params(pp)
    } else {
        match (config.evolve_signals, config.evolve_patterns) {
            (true, true) => GenomeSpec::combined_with_patterns_spec(config.crypto),
            (true, false) => GenomeSpec::combined_spec(config.crypto),
            (false, true) => GenomeSpec::params_with_patterns_spec(config.crypto),
            (false, false) => GenomeSpec::params_spec(config.crypto),
        }
    };

    let is_dynamic = pipeline_params.is_some();

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
                is_dynamic,
                pipeline_params.as_ref(),
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
                is_dynamic,
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

/// Try to load the `params` block from a dynamic JSON strategy config.
///
/// Returns `None` for hardcoded strategy names or if the JSON file is missing.
fn load_pipeline_params(
    setup_name: &str,
) -> Option<std::collections::HashMap<String, engine_pipeline::config::ParamValue>> {
    match setup_name {
        "breakout" | "ep" | "parabolic" | "signal_breakout" | "pattern_breakout" => None,
        _ => {
            let candidates = crate::strategy::dynamic_strategy_paths(setup_name);
            for path in &candidates {
                if path.exists() {
                    if let Ok(json_str) = std::fs::read_to_string(path) {
                        if let Ok(pipeline) =
                            serde_json::from_str::<engine_pipeline::config::StrategyPipeline>(
                                &json_str,
                            )
                        {
                            if !pipeline.params.is_empty() {
                                return Some(pipeline.params);
                            }
                        }
                    }
                }
            }
            None
        }
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
// Shared: parallel evaluation + sort
// ---------------------------------------------------------------------------

/// Evaluate a population in parallel and return `(genomes, scores, best_report)`
/// sorted best-first by fitness.
///
/// When `is_dynamic` is true, each genome is decoded to a JSON override map
/// and the DynamicSetup is re-created with overridden params per evaluation.
fn evaluate_population(
    store: &DataStore,
    base: &Params,
    config: &EvolutionConfig,
    spec: &GenomeSpec,
    population: Vec<Vec<f64>>,
    is_dynamic: bool,
) -> (Vec<(Vec<f64>, f64)>, serde_json::Value) {
    let results: Vec<(serde_json::Value, f64)> = population
        .par_iter()
        .map(|genome| {
            if is_dynamic {
                let overrides = spec.decode_to_overrides(genome);
                evaluate_dynamic(
                    store,
                    base,
                    &overrides,
                    &config.fitness_metric,
                    &base.fitness,
                )
            } else {
                let params = spec.decode(genome, base);
                evaluate(store, &params, &config.fitness_metric, &base.fitness)
            }
        })
        .collect();

    let mut scored: Vec<(Vec<f64>, f64)> = population
        .into_iter()
        .zip(results.iter().map(|(_, f)| *f))
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let best_report = results
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(r, _)| r.clone())
        .unwrap_or_default();

    (scored, best_report)
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
    is_dynamic: bool,
    pipeline_params: Option<&std::collections::HashMap<String, engine_pipeline::config::ParamValue>>,
) {
    let initial_mean = if let Some(pp) = pipeline_params {
        spec.encode_from_pipeline_params(pp)
    } else {
        spec.encode(base)
    };
    let mut optimizer = cmaes::CmaEs::new(spec.dim(), initial_mean, config.initial_sigma, base.cmaes.clone());

    for gen in 0..config.generations {
        let population = optimizer
            .ask(rng)
            .into_iter()
            .map(|mut g| { spec.clamp(&mut g); g })
            .collect();

        let (scored, best_report) = evaluate_population(store, base, config, spec, population, is_dynamic);

        record_generation(gen, config, spec, base, &scored, &best_report,
            &optimizer.sigma(), history, best_fitness, best_params_json, best_report_json, is_dynamic);

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
    is_dynamic: bool,
) {
    let pop_size = if config.pop_size > 0 { config.pop_size } else { 100 };
    let mut optimizer = ga::Ga::new(spec.dim(), pop_size, 0.25, base.ga.clone());

    let bounds: Vec<(f64, f64, bool, bool)> = spec
        .bounds
        .iter()
        .map(|b| (b.min, b.max, b.is_integer, b.is_boolean))
        .collect();

    let mut population: Vec<Vec<f64>> = (0..pop_size).map(|_| spec.random(rng)).collect();

    for gen in 0..config.generations {
        let (scored, best_report) = evaluate_population(store, base, config, spec, population, is_dynamic);

        record_generation(gen, config, spec, base, &scored, &best_report,
            &optimizer.sigma(), history, best_fitness, best_params_json, best_report_json, is_dynamic);

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
    is_dynamic: bool,
) {
    let gen_best = scored[0].1;
    let gen_mean = scored.iter().map(|(_, f)| f).sum::<f64>() / scored.len() as f64;

    let improved = gen_best > *best_fitness;
    if improved {
        *best_fitness = gen_best;
        let best_genome = &scored[0].0;
        if is_dynamic {
            // Dynamic pipelines: gene names are JSON param keys, not Params fields.
            *best_params_json =
                serde_json::to_value(spec.decode_to_overrides(best_genome)).unwrap_or_default();
        } else {
            *best_params_json =
                serde_json::to_value(spec.decode(best_genome, base)).unwrap_or_default();
        }
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
