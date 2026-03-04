# Evolution Orchestration Module

## Task

Create the evolution orchestration layer (`src/evolution/mod.rs`) and wire it
into the CLI and main dispatch for the algotrader engine.

## Files Changed

### Created

- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/evolution/mod.rs`
  -- New orchestration module (465 lines). Defines public types (`Algorithm`,
  `EvolutionConfig`, `GenerationLog`, `EvolutionResult`,
  `WalkForwardEvolutionResult`, `WfFold`) and two entry points: `evolve()`
  for standard runs and `evolve_walk_forward()` for time-series
  cross-validated evolution. Delegates to private `run_cmaes()` and `run_ga()`
  helpers that share a `record_generation()` function for logging and
  best-tracking.

### Modified

- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/lib.rs`
  -- Added `pub mod evolution;` to the module list (alphabetical order).

- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/cli.rs`
  -- Added 10 CLI flags (`--evolve`, `--algo`, `--generations`,
  `--pop-size-evo`, `--fitness`, `--sigma`, `--evolve-signals`, `--crypto`,
  `--wf-folds`, `--seed`) grouped under a comment block after existing fields.

- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/main.rs`
  -- Added evolution dispatch branch before the serve/batch/single branches.
  Constructs `EvolutionConfig` from CLI args and calls either
  `evolve_walk_forward()` or `evolve()`, printing JSON results to stdout.

## Design Decisions

1. **Shared `record_generation` helper** -- The original plan had duplicated
   generation-logging code in both the CMA-ES and GA branches. Extracted into
   a single `record_generation()` function to keep things DRY. The function
   accepts mutable references to the running best-state and history vector.

2. **Removed unused import** -- The plan imported `extract_fitness` from the
   fitness module, but it is only used internally within `evaluate()`. Removed
   the dead import.

3. **Private dispatch functions** -- Split `run_cmaes` and `run_ga` into
   separate private functions instead of inlining them in `evolve()`. This
   keeps the public `evolve()` function at a manageable size and makes each
   optimizer's loop independently readable.

4. **Pending sibling modules** -- `mod.rs` declares `pub mod cmaes;` and
   `pub mod ga;` which do not exist on disk yet. These will be created by
   parallel agents. The code will not compile until those modules are in place.

## Status

Went according to plan with minor clean-code improvements (DRY helper, dead
import removal). No blockers.
