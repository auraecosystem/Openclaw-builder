# Evolution Config Call-Site Wiring Report

## Task
Replace hardcoded numeric constants in the evolution module with reads from the `FitnessConfig`, `GaConfig`, and `CmaEsConfig` structs that were previously created and wired into `Params`.

## Files Modified

### `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/evolution/fitness.rs`
- Added `use super::fitness_config::FitnessConfig` import.
- `evaluate()` gained a `fitness_cfg: &FitnessConfig` parameter, passed through to `extract_fitness()`.
- `extract_fitness()` gained a `cfg: &FitnessConfig` parameter.
- Replaced 11 hardcoded literals: `min_trades_gate` (10), `penalty_score` (-999.0), `confidence_base` (0.3), `confidence_slope` (0.7), `confidence_range` (40.0), `sharpe_cap_base` (3.5), `composite_w_sharpe` (0.4), `composite_w_return` (0.3), `composite_w_winrate` (0.3), `composite_return_scale` (10.0), `drawdown_penalty_mult` (2.0).
- Updated all 9 test functions to pass `&FitnessConfig::default()`.

### `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/evolution/ga.rs`
- Added `use super::ga_config::GaConfig` import.
- `Ga::new()` gained a `config: GaConfig` parameter (owned). Stored in struct, replaced `base_sigma`/`stale_limit`/`cataclysm_limit` fields with config reads.
- Replaced 10 hardcoded literals in `evolve_step()`: `base_sigma` (0.15), `stale_sigma_step` (0.3), `cataclysm_limit` (12), `cataclysm_random_frac` (0.5), `cataclysm_sigma_mult` (2.0), `stale_limit` (5), `mild_injection_frac` (0.3), `tournament_k` (3), `mutation_rate` (0.3), `crossover_prob` (0.5).
- Updated `crossover()` to accept a `prob: f64` parameter instead of hardcoded 0.5.
- Updated all 5 test functions to pass `GaConfig::default()`.

### `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/evolution/cmaes.rs`
- Added `use super::cmaes_config::CmaEsConfig` import.
- `CmaEs::new()` gained a `config: CmaEsConfig` parameter (owned). Stored in struct.
- Replaced 2 hardcoded literals: `sigma_clamp_hi` (1e2) and `decomp_interval_div` (10).
- Updated all 4 test functions to pass `CmaEsConfig::default()`.

### `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/evolution/mod.rs`
- Updated `run_cmaes()`: passes `base.cmaes.clone()` to `CmaEs::new()`, `&base.fitness` to `evaluate()`.
- Updated `run_ga()`: passes `base.ga.clone()` to `Ga::new()`, `&base.fitness` to `evaluate()`.
- Updated `evolve_walk_forward()`: passes `&best_params.fitness` to `evaluate()`.

## Outcome
- All 26 evolution module tests pass.
- `cargo check` succeeds with no new errors.
- Behavior is unchanged since all config defaults match the previously hardcoded values.
