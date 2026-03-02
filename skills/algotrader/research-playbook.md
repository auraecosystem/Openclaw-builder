# Research Playbook — Step-by-Step Methodology

## Phase 0: Session Initialization

Before any research, establish context:

1. **Read the lab notebook** (`lab-notebook.jsonl`). Parse the last 10–20 entries. Identify:
   - The most recent experiment and its conclusion
   - The `next_steps` field from the last entry — this is your starting point
   - Any strategies currently promoted to validation or beyond
   - Failed hypothesis families (to avoid re-testing)

2. **Read session memory** if available. Check for:
   - Parameter regions already explored
   - Strategies in the promotion pipeline
   - Infrastructure notes (engine bugs, data issues)

3. **Verify NautilusTrader is working** — run a minimal backtest:
   ```python
   from nautilus_trader.backtest.engine import BacktestEngine
   from nautilus_trader.backtest.config import BacktestEngineConfig
   engine = BacktestEngine(config=BacktestEngineConfig())
   # Quick smoke test: add venue + instrument + data + strategy, run()
   ```

4. **Verify data is loadable** — check that parquet files load correctly:
   ```python
   import pandas as pd
   df = pd.read_parquet("data-crypto/ohlcv_daily.parquet")
   print(f"Shape: {df.shape}, Date range: {df.index.min()} to {df.index.max()}")
   ```

---

## Phase 1: Hypothesis Generation

Sources of hypotheses, in priority order:

### 1. Prior Experiment Conclusions (Highest Priority)
The `next_steps` field from the most recent experiment is the most valuable source. It represents the research thread you're actively pursuing. Follow it.

### 2. Reference Materials
Re-read a reference document and extract a testable claim. Examples:
- "VCP patterns with 3+ contractions should outperform 2-contraction VCPs" (from `qullamaggie-rules.md`)
- "Regime filter should improve profit factor but reduce trade count" (test the `regime` parameter)
- "Tighter consolidation ranges (<10%) should produce better breakouts" (test `max_range_pct`)
- "EP setups with prior 6M return < 10% (more neglected) outperform those already up 10-30%" (from behavioral bias theory)

### 3. Parameter Sensitivity
Pick a parameter and ask: is the current default optimal? Is the response surface flat (insensitive = robust) or steep (sensitive = fragile)?

Good candidates for exploration:
- `rs_pct` — how selective should RS ranking be?
- `vol_ratio` — how strong must the volume spike be?
- `min_consol_days` — longer consolidation = better base?
- `split_frac` — optimal quick/runner ratio?
- `slippage_k` — how sensitive is the edge to cost assumptions?

### 4. Structural Modifications
Hypothesize about new filters, exit rules, or entry timing changes. These require careful reasoning about *why* they should work (behavioral or structural mechanism):
- "Adding a sector-relative strength filter would improve breakout quality"
- "Trailing stop at SMA_20 instead of SMA_10 for runners would capture more trend"
- "Requiring prior earnings beat as EP qualifier would improve win rate"

### 5. Cross-Asset Transfer
"Does the breakout setup that works on US stocks also work on crypto?" Test with appropriate parameter adjustments (wider RS, no penny filter, different ADV ranges).

### Hypothesis Quality Checklist

Before proceeding, the hypothesis must pass ALL of these:
- [ ] **Falsifiable?** Can I define a specific metric threshold that rejects it?
- [ ] **Plausible mechanism?** Why would this work? Who is on the other side of the trade?
- [ ] **Testable with current infrastructure?** No new code needed, or minimal changes?
- [ ] **Sufficiently narrow?** Tests ONE thing, not five things at once?
- [ ] **Not already tested?** Check lab notebook for prior experiments on this exact hypothesis.

---

## Phase 2: Experiment Design

**Before running any backtest**, fill out the experiment header in the lab notebook.

Define these elements:

| Element | Description | Example |
|---------|-------------|---------|
| **Control** | Baseline to compare against | Default params on training set |
| **Treatment** | The specific change being tested | `max_range_pct` changed from 0.15 to 0.10 |
| **Primary metric** | Which metric determines success (pick ONE) | `profit_factor` |
| **Success threshold** | What value constitutes meaningful improvement | Treatment PF > Control PF × 1.10 |
| **Minimum trades** | Minimum sample size for statistical significance | 30 trades |
| **Date range** | Which data split | Training: start to 2021-12-31 (crypto) |

For optimization experiments, additionally define:
- Which parameters are being optimized? (Max 5)
- Which are held fixed?
- What is the fitness metric?

**Write the lab notebook entry header NOW** (with `status: "running"`). Then execute.

---

## Phase 3: Execution

**Rule: every backtest run MUST call `log_run()` immediately after `extract_metrics()`.** See `experiment-templates.md` for `log_run()` and `extract_metrics()` definitions. The log file (`experiment-runs.jsonl`) captures full params + metrics for every run — control, treatment, sweep points, WFA windows, validation, holdout. No exceptions, including rejected experiments.

### Single Parameter Test
```python
from nautilus_trader.backtest.engine import BacktestEngine
from nautilus_trader.backtest.config import BacktestEngineConfig

# Control (defaults):
control_config = BreakoutConfig(
    instrument_id=instrument_id,
    bar_type=bar_type,
    # ... default params
)
engine = BacktestEngine(config=BacktestEngineConfig(...))
# add venue, instrument, data, strategy
engine.add_strategy(BreakoutStrategy(control_config))
engine.run()
control_results = extract_metrics(engine)
log_run("EXP-NNN", control_config.__dict__, control_results, label="control")
engine.dispose()

# Treatment (one param changed):
treatment_config = BreakoutConfig(
    instrument_id=instrument_id,
    bar_type=bar_type,
    max_range_pct=0.10,  # changed from 0.15
    # ... other params same as control
)
engine = BacktestEngine(config=BacktestEngineConfig(...))
engine.add_strategy(BreakoutStrategy(treatment_config))
engine.run()
treatment_results = extract_metrics(engine)
log_run("EXP-NNN", treatment_config.__dict__, treatment_results, label="treatment")
engine.dispose()
```

### Parameter Sweep
Loop over values and collect results:
```python
results = []
for max_range_pct in [0.05, 0.08, 0.10, 0.12, 0.15, 0.18, 0.20, 0.25, 0.30]:
    config = BreakoutConfig(max_range_pct=max_range_pct, ...)
    engine = BacktestEngine(config=BacktestEngineConfig(...))
    # add venue, instrument, data
    engine.add_strategy(BreakoutStrategy(config))
    engine.run()
    metrics = extract_metrics(engine)
    results.append({"max_range_pct": max_range_pct, **metrics})
    log_run("EXP-NNN", config.__dict__, metrics, label=f"sweep-max_range_pct={max_range_pct}")
    engine.dispose()

# Tabulate: look for smooth optimum (robust) vs. spiky peak (fragile)
pd.DataFrame(results).to_string()
```

### Multi-Parameter Optimization
Loop over parameter grid (max 5 free parameters):
```python
import itertools

param_grid = {
    "rs_pct": [0.10, 0.20, 0.30, 0.40],
    "risk_pct": [0.01, 0.02, 0.03],
    "split_frac": [0.0, 0.25, 0.50],
    "rs_lookback": [7, 14, 21],
}

results = []
for combo in itertools.product(*param_grid.values()):
    params = dict(zip(param_grid.keys(), combo))
    config = BreakoutConfig(**params, ...)
    engine = BacktestEngine(config=BacktestEngineConfig(...))
    engine.add_strategy(BreakoutStrategy(config))
    engine.run()
    metrics = extract_metrics(engine)
    results.append({**params, **metrics})
    log_run("EXP-NNN", {**params}, metrics, label=f"grid-{params}")
    engine.dispose()
```

**Important**: Always restrict date ranges to the training period. Never optimize on the full dataset.

After optimization completes:
1. Record the best params
2. Immediately run parameter sensitivity (±20% on each optimized param)
3. Check if Sharpe > 3.0 — if so, investigate overfitting before proceeding

### Walk-Forward Analysis

Run optimization on each rolling in-sample window, test best params on subsequent out-of-sample window.

**Crypto rolling windows** (12m IS / 3m OOS, rolling by 3m):

| Window | In-Sample | Out-of-Sample |
|--------|-----------|---------------|
| 1 | 2018-01 to 2018-12 | 2019-01 to 2019-03 |
| 2 | 2018-04 to 2019-03 | 2019-04 to 2019-06 |
| 3 | 2018-07 to 2019-06 | 2019-07 to 2019-09 |
| ... | ... | ... |
| N | 2021-01 to 2021-12 | (end of training set) |

**Stock rolling windows** (24m IS / 6m OOS, rolling by 6m):

| Window | In-Sample | Out-of-Sample |
|--------|-----------|---------------|
| 1 | 1997-01 to 1998-12 | 1999-01 to 1999-06 |
| 2 | 1997-07 to 1999-06 | 1999-07 to 1999-12 |
| ... | ... | ... |

For each window:
1. Run grid optimization on the IS period
2. Log IS best run: `log_run("EXP-NNN", best_params, is_metrics, label=f"wfa-window-{i}-is")`
3. Run those exact params on the OOS period via a single backtest
4. Log OOS run: `log_run("EXP-NNN", best_params, oos_metrics, label=f"wfa-window-{i}-oos")`

Calculate:
```
WFE = mean(OOS Sharpe across all windows) / mean(IS Sharpe across all windows)
```

### Sub-Period Analysis

**Crypto** — split training set into three periods:
```python
# Period 1: Early crypto (pre-2019)
# Period 2: 2019-2020 (includes COVID crash)
# Period 3: 2020-2021 (bull run to end of training)
```

**Stocks** — split training set into three periods:
```python
# Period 1: 1997-2003 (includes dot-com crash)
# Period 2: 2004-2009 (includes 2008 financial crisis)
# Period 3: 2010-2015 (bull market recovery)
```

---

## Phase 4: Analysis

Apply these gates IN ORDER. Stop at the first failure.

### Gate 1: Minimum Viable Signal
- Total trades >= 30? If NO: **REJECT** immediately.
- Profit factor > 1.0? If NO: **REJECT** immediately.

### Gate 2: Statistical Significance
- Is the improvement meaningful? For parameter changes: improvement > 10% on primary metric. Smaller improvements are likely noise.
- Is trade count sufficient for the holding period? (See guardrails for minimum sample sizes.)

### Gate 3: Overfitting Red Flags
- Sharpe > 3.5? **REJECT** (hard stop).
- Sharpe > 2.5? **YELLOW FLAG** — proceed only with extra robustness checks.
- More than 5 optimized parameters? **RED FLAG**.
- Performance collapses when perturbing params ±20%? **RED FLAG**.
- Strategy only works in one sub-period? **RED FLAG**.

### Gate 4: Robustness (requires Gates 1–3 to pass)
- **Sub-period**: PF > 1.0 in at least 2 of 3 sub-periods.
- **Parameter sensitivity**: perturb each optimized parameter ±20%. Sharpe should not drop >30%.
- **Cost sensitivity**: increase `slippage_k` by 50% (e.g., 0.10 → 0.15). Strategy must remain profitable (PF > 1.0).

### Gate 5: Walk-Forward Validation (requires Gate 4)
- WFE > 0.50: **PASS**
- WFE 0.30–0.50: **MARGINAL** — try reducing parameter count and retesting
- WFE < 0.30: **REJECT**

### Gate 6: Validation Set (requires Gate 5)
- Crypto: run on 2022-01-01 to 2023-06-30 with optimized params from WFA
- Stocks: run on 2016-01-01 to 2020-12-31 with optimized params from WFA
- Validation Sharpe should be > 0.5 × Training Sharpe
- PF > 1.2, MDD < 30%
- If validation fails: **REJECT**. Do NOT touch holdout. Do NOT re-optimize.

### Gate 7: Holdout — FINAL, ONE-TIME ONLY (requires Gate 6)
- Crypto: run on 2023-07-01 to present
- Stocks: run on 2021-01-01 to 2024-12-31
- Result is FINAL. No re-optimization after seeing this.
- Thresholds: Sharpe > 1.0, PF > 1.3, MDD < 25%, trades >= 30
- **PASS** → Strategy is VIABLE for paper trading
- **FAIL** → Strategy is REJECTED. Start over with new hypothesis.

---

## Phase 5: Recording

After every experiment — pass or fail:

1. **Verify run log completeness**: check that `experiment-runs.jsonl` has entries for every run in this experiment (control, treatment, every sweep point, every WFA window). If any are missing, reconstruct and append them now. The run log is the ground truth for reproducibility.

2. Update the lab notebook entry:
   - Set `status` to `"rejected"`, `"promoted"`, or `"needs_further_testing"`
   - Fill in all result metrics
   - Reference the run log: `"run_log": "experiment-runs.jsonl"` with the experiment ID for lookup
   - Write a 2–3 sentence `conclusion`
   - Write 1–3 `next_steps` items

3. If this was a validation or holdout test, link back to the original training experiment via `parent_id`.

4. Update session memory with:
   - Parameter regions explored
   - Strategies promoted (and to which stage)
   - Failed hypothesis families

---

## Phase 6: Iteration

Read the `next_steps` from the just-completed experiment and begin Phase 1 again.

### When to Change Direction

- After **3 consecutive rejections** on the same hypothesis family: step back and reconsider the fundamental assumption.
- After **5 consecutive rejections** across different hypotheses: re-read reference materials for fresh ideas.
- If all breakout-related hypotheses are exhausted: switch to EP or parabolic setups, or cross-asset testing.

### When to Declare Victory

A strategy is "found" when it passes all 7 gates AND:
- Walk-forward efficiency > 0.50
- Holdout Sharpe > 1.0 (net of costs)
- Max drawdown < 25% on holdout
- Profit factor > 1.3 on holdout
- Profitable in >= 2 of 3 sub-periods of training data
- The mechanism is explainable in plain English

This strategy is then documented in `strategy-registry.json` and is ready for paper trading validation.

### Session End Checklist

Before ending any research session:
- [ ] All experiments recorded in lab notebook
- [ ] All backtest runs logged in `experiment-runs.jsonl` (full params + metrics)
- [ ] Session memory updated with key findings
- [ ] Strategy registry updated if any promotions occurred
- [ ] The last notebook entry has clear `next_steps` for the next session
