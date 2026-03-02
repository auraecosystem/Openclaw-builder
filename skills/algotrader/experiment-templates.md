# Experiment Templates

Eight templates for different types of experiments. Each template defines the steps, typical duration, and guardrails specific to that experiment type.

---

## Mandatory Logging

**Every experiment run MUST log its full parameter set and output metrics.** Use `log_run()` after every backtest — no exceptions, including failed/rejected runs.

```python
import json, datetime
from pathlib import Path

def log_run(experiment_id: str, params: dict, metrics: dict, *,
            label: str = "", notes: str = "",
            log_path: str = "experiment-runs.jsonl"):
    """Append one run's full params + metrics to the experiment log."""
    entry = {
        "experiment_id": experiment_id,
        "timestamp": datetime.datetime.utcnow().isoformat() + "Z",
        "label": label,          # e.g. "control", "treatment", "sweep-rs_pct=0.04"
        "params": params,         # FULL config dict (not just changed params)
        "metrics": metrics,       # FULL metrics dict from extract_metrics()
        "notes": notes,
    }
    with open(log_path, "a") as f:
        f.write(json.dumps(entry, default=str) + "\n")
```

### What to log

| Field | Required | Example |
|---|---|---|
| `experiment_id` | Yes | `"EXP-005"` |
| `label` | Yes | `"control"`, `"treatment"`, `"sweep-rs_pct=0.04"`, `"wfa-window-3-is"` |
| `params` | Yes | Full `config.__dict__` or `config.to_dict()` — every param, not just changed ones |
| `metrics` | Yes | `{"trades": 208, "win_rate": 0.34, "pf": 1.44, "sharpe": 0.91, "sortino": 1.12, "mdd": 0.18, "cagr": 0.28, "avg_hold_bars": 14}` |
| `notes` | No | `"baseline re-run after fixing stop logic"` |

### extract_metrics() reference

```python
def extract_metrics(engine) -> dict:
    """Extract standard metrics from a completed BacktestEngine."""
    report = engine.trader.generate_order_fills_report()
    account = engine.trader.generate_account_report()
    # Use engine.trader.generate_positions_report() for position-level stats
    stats = engine.kernel.get_stats()
    return {
        "trades": stats.get("total_trades", 0),
        "win_rate": stats.get("win_rate", 0.0),
        "pf": stats.get("profit_factor", 0.0),
        "sharpe": stats.get("sharpe_ratio", 0.0),
        "sortino": stats.get("sortino_ratio", 0.0),
        "mdd": stats.get("max_drawdown", 0.0),
        "cagr": stats.get("cagr", 0.0),
        "avg_hold_bars": stats.get("avg_duration", 0),
        "total_return": stats.get("total_return", 0.0),
        "avg_winner": stats.get("avg_winner", 0.0),
        "avg_loser": stats.get("avg_loser", 0.0),
    }
```

---

## Template 1: Single Parameter Test

**Purpose**: Test whether changing one parameter improves results.
**Complexity**: Low. Good starting point for new hypotheses.

### Steps

1. Define hypothesis: "Changing parameter X from A to B improves metric Y because [mechanism]."
2. Run **control** (defaults) on training set:
   ```python
   control_config = BreakoutConfig(instrument_id=..., bar_type=..., ...)
   engine = BacktestEngine(config=BacktestEngineConfig(...))
   # add venue, instrument, data
   engine.add_strategy(BreakoutStrategy(control_config))
   engine.run()
   control_results = extract_metrics(engine)
   engine.dispose()
   ```
3. Run **treatment** (one param changed) on training set:
   ```python
   treatment_config = BreakoutConfig(max_range_pct=0.10, ...)  # one change
   engine = BacktestEngine(config=BacktestEngineConfig(...))
   engine.add_strategy(BreakoutStrategy(treatment_config))
   engine.run()
   treatment_results = extract_metrics(engine)
   engine.dispose()
   ```
4. **Log both runs** (control and treatment):
   ```python
   log_run("EXP-NNN", control_config.__dict__, control_results, label="control")
   log_run("EXP-NNN", treatment_config.__dict__, treatment_results, label="treatment")
   ```
5. Compare primary metric. If improvement > 10%: promote to parameter sweep or sub-period analysis.
6. Record in lab notebook regardless of outcome.

### Guardrails
- Change only ONE parameter between control and treatment.
- If trade count drops >50%, the parameter may be too restrictive — note this in the conclusion.

---

## Template 2: Parameter Sweep

**Purpose**: Map the response surface of one parameter across its full range.
**Complexity**: Low-Medium. Run after a single parameter test shows promise.

### Steps

1. Generate 10–20 evenly spaced values across the parameter's valid range.
2. Loop over values, running a backtest for each:
   ```python
   results = []
   for rs_pct in [0.01, 0.02, 0.04, 0.06, 0.08, 0.10, 0.15, 0.20]:
       config = BreakoutConfig(rs_pct=rs_pct, ...)
       engine = BacktestEngine(config=BacktestEngineConfig(...))
       engine.add_strategy(BreakoutStrategy(config))
       engine.run()
       metrics = extract_metrics(engine)
       results.append({"rs_pct": rs_pct, **metrics})
       log_run("EXP-NNN", {"rs_pct": rs_pct, **config.__dict__}, metrics,
               label=f"sweep-rs_pct={rs_pct}")
       engine.dispose()
   ```
3. Tabulate results: parameter value vs. each metric (Sharpe, PF, trades, MDD).
4. Identify:
   - Is there a clear optimum? Or is the surface flat?
   - **Smooth surface with broad optimum** = robust. Good sign.
   - **Spiky surface with narrow peak** = fragile. Overfitting risk.
5. Record the optimal value and note whether the surface is smooth or spiky.
6. If smooth: run parameter sensitivity (±20%) on the optimal value to confirm.

### Output Format
Present results as a table:
```
rs_pct | trades | win_rate | PF   | sharpe | MDD
0.01   | 312    | 0.29     | 1.62 | 1.45   | 0.21
0.02   | 487    | 0.31     | 1.45 | 1.23   | 0.19
...
```

---

## Template 3: Multi-Parameter Optimization

**Purpose**: Find optimal parameter combinations across multiple parameters.
**Complexity**: Medium-High. **Use sparingly** — high overfitting risk.

### Steps

1. Decide which parameters to optimize (**MAX 5**). Fix all others at defaults.
2. Define parameter grid:
   ```python
   import itertools

   param_grid = {
       "rs_pct": [0.10, 0.20, 0.30, 0.40],
       "risk_pct": [0.01, 0.02, 0.03],
       "split_frac": [0.0, 0.25, 0.50],
       "rs_lookback": [7, 14, 21],
   }
   ```
3. Run grid search on training set:
   ```python
   results = []
   for combo in itertools.product(*param_grid.values()):
       params = dict(zip(param_grid.keys(), combo))
       config = BreakoutConfig(**params, ...)
       engine = BacktestEngine(config=BacktestEngineConfig(...))
       engine.add_strategy(BreakoutStrategy(config))
       engine.run()
       metrics = extract_metrics(engine)
       results.append({**params, **metrics})
       log_run("EXP-NNN", {**params}, metrics,
               label=f"grid-{params}")
       engine.dispose()

   # Sort by fitness metric
   best = sorted(results, key=lambda r: r["sharpe"], reverse=True)[0]
   ```
4. Record best params AND note the diversity of top-10 results.
5. **Immediately** run parameter sensitivity on best params (±20% on each optimized param).
6. If sensitivity passes: run sub-period analysis.
7. If sub-period passes: proceed to walk-forward analysis.

### Guardrails
- **NEVER** optimize more than 5 parameters. More degrees of freedom = more overfitting.
- **ALWAYS** run sensitivity test immediately after. If any param's ±20% perturbation drops Sharpe >30%, the result is fragile.
- If best Sharpe > 3.0: **STOP**. Investigate overfitting before proceeding.
- Check diversity: if top-10 results all cluster at nearly identical params = overfit risk.

---

## Template 4: Walk-Forward Analysis

**Purpose**: Validate that a strategy generalizes to unseen data.
**Complexity**: High. The most important validation step.

### Steps

1. Define rolling windows:

   **Crypto** (12m IS / 3m OOS, rolling by 3m):

   | Window | In-Sample | Out-of-Sample |
   |--------|-----------|---------------|
   | 1 | 2018-01 to 2018-12 | 2019-01 to 2019-03 |
   | 2 | 2018-04 to 2019-03 | 2019-04 to 2019-06 |
   | 3 | 2018-07 to 2019-06 | 2019-07 to 2019-09 |
   | ... | (roll by 3 months) | ... |

   **Stocks** (24m IS / 6m OOS, rolling by 6m):

   | Window | In-Sample | Out-of-Sample |
   |--------|-----------|---------------|
   | 1 | 1997-01 to 1998-12 | 1999-01 to 1999-06 |
   | 2 | 1997-07 to 1999-06 | 1999-07 to 1999-12 |
   | ... | (roll by 6 months) | ... |

2. For each window:
   a. Run grid optimization on the IS period (filter data by date range)
   b. Log IS best run: `log_run("EXP-NNN", best_params, is_metrics, label=f"wfa-window-{i}-is")`
   c. Run those exact params on the OOS period via a single backtest
   d. Log OOS run: `log_run("EXP-NNN", best_params, oos_metrics, label=f"wfa-window-{i}-oos")`

3. Calculate WFE:
   ```
   WFE = mean(all OOS Sharpes) / mean(all IS Sharpes)
   ```

4. Decision:
   - WFE > 0.50: **PASS**. Proceed to validation set.
   - WFE 0.30–0.50: **MARGINAL**. Consider reducing parameter count and retesting.
   - WFE < 0.30: **FAIL**. Strategy is curve-fitted. Reject.

### Guardrails
- Windows must be **non-overlapping** in OOS periods (IS can overlap).
- Minimum 5 OOS windows for statistical meaning.
- Record the Sharpe of each individual window — check for outlier windows that dominate the average.

---

## Template 5: Sub-Period Analysis

**Purpose**: Check that performance isn't concentrated in one lucky era.
**Complexity**: Low. Fast check. Always run before WFA.

### Steps

1. Split training set into three periods:

   **Crypto:**
   - **Period 1**: Pre-2019 (early market, less mature)
   - **Period 2**: 2019-2020 (includes COVID crash, DeFi summer)
   - **Period 3**: 2020-2021 (bull run to end of training)

   **Stocks:**
   - **Period 1**: 1997-2003 (includes dot-com crash)
   - **Period 2**: 2004-2009 (includes 2008 financial crisis)
   - **Period 3**: 2010-2015 (bull market recovery)

2. Run the strategy on each sub-period independently with the same params.

3. Assess:
   - All 3 must have PF > 1.0
   - At least 2 of 3 must have Sharpe > 0.5
   - If the strategy **only** works in Period 3 (bull market): **RED FLAG** — it may be capturing beta, not alpha.

### Output Format
```
Period   | Dates           | Trades | PF   | Sharpe | MDD
Period 1 | pre-2019        | 45     | 1.23 | 0.87   | 0.22
Period 2 | 2019-2020       | 62     | 1.15 | 0.62   | 0.31
Period 3 | 2020-2021       | 101    | 1.78 | 1.54   | 0.14
```

---

## Template 6: Cross-Asset Transfer

**Purpose**: Test whether a strategy that works on stocks also works on crypto (or vice versa).
**Complexity**: Medium. Requires adjusting parameter ranges for different market structure.

### Steps

1. Take the best parameters from stock testing.
2. Adjust ranges for crypto:
   - `min_adv`: 0 (crypto volume scale is different)
   - `rs_pct`: 0.20–0.60 (only 100 tickers; wider RS needed)
   - `min_adr_pct`: 0–0.05 (crypto is already volatile)
   - `min_price`: 0 (no penny stock filter)
   - `min_vol`: 0
3. Load crypto data and run on crypto training set:
   ```python
   crypto_df = pd.read_parquet("data-crypto/ohlcv_daily.parquet")
   # Filter to training period: start to 2021-12-31
   # Wrangle into NautilusTrader Bar objects
   # Run backtest with adjusted params
   ```
4. If profitable: run optimization on crypto training set.
5. Compare crypto-optimized params to stock params:
   - **Large divergence** = different market character (expected and fine)
   - **Some overlap** = genuine cross-asset signal (stronger evidence of real edge)

---

## Template 7: Regime Analysis

**Purpose**: Understand which market conditions favor the strategy.
**Complexity**: Medium. Important for portfolio construction and regime filtering decisions.

### Steps

1. Classify training-set periods into market regimes:

   **Crypto:**
   | Regime | Periods |
   |--------|---------|
   | Bull trending | 2017, late-2020–2021 |
   | Bear trending | 2018, early-2020 (COVID) |
   | High volatility | 2017-2018, 2020-2021 |
   | Low volatility | 2019 |

   **Stocks:**
   | Regime | Periods |
   |--------|---------|
   | Bull trending | 2003–2007, 2010–2015 |
   | Bear trending | 2000–2002, 2008–2009 |
   | High volatility | 2000–2002, 2008–2009 |
   | Low volatility | 2005–2006, 2013–2014 |

2. Run the strategy on each regime-period.
3. Record performance per regime.
4. Analysis:
   - If profitable **only** in bull regimes: may be capturing beta, not alpha. Test with `regime: true` to see if the BTC > 200d SMA filter (crypto) or SPY > 200d SMA filter (stocks) helps.
   - If profitable across multiple regimes: stronger evidence of real edge.
   - If profitable in bear/crisis regimes: especially valuable for portfolio diversification.

### Compare regime=true vs. regime=false
```python
# Regime ON:
config_on = BreakoutConfig(regime=True, ...)
# Run backtest, extract metrics

# Regime OFF:
config_off = BreakoutConfig(regime=False, ...)
# Run backtest, extract metrics
```

---

## Template 8: New Filter or Exit Rule

**Purpose**: Test a structural modification to the strategy logic (not just parameter tuning).
**Complexity**: High. Requires code changes to the NautilusTrader strategy class.

### Steps

1. **State the change mechanistically**: what exactly does the new filter/rule do?
2. **Explain the mechanism**: WHY should this improve results? What behavioral or structural force does it exploit?
3. If it requires strategy code changes:
   a. Define the change precisely (which method, what logic)
   b. Implement in the Strategy subclass (e.g., `BreakoutStrategy.on_bar()`)
   c. Add new config fields to the `StrategyConfig` dataclass
   d. Test that the strategy loads and runs without errors
4. Run control (without new filter) and treatment (with new filter) on training set.
5. Compare metrics.
6. If treatment wins: proceed through the standard validation pipeline (sub-period → optimization → WFA → validation → holdout).

### Guardrails
- New filters should be **simple** — one additional condition, not a complex multi-condition chain.
- The filter should be parameterizable (so it can be tested across a range, not just on/off).
- Always verify the new code is correct by checking that trade count changed in the expected direction.

### Examples of Structural Changes
- Adding a sector-relative strength filter (not just absolute RS)
- Changing exit rule from SMA_10 to SMA_20 trailing stop
- Adding a volume confirmation on the exit (sell only on above-average volume)
- Adding a consecutive-days-in-base requirement
- Implementing a different stop-loss methodology (e.g., chandelier stop instead of LOD)
