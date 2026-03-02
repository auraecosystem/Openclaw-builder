# Lab Notebook Schema

## Storage

**File**: `lab-notebook.jsonl` — JSON Lines format, one experiment per line, **append-only**.

Never delete or modify existing entries. Corrections are recorded as new entries referencing the original via `corrects_id`.

---

## Full Schema

```json
{
  "id": "EXP-001",
  "date": "2026-02-26",
  "status": "rejected",
  "parent_id": null,
  "corrects_id": null,

  "hypothesis": "Tighter consolidation range (max_range_pct=0.10 vs 0.15 default) produces higher profit factor for breakout setups on crypto, because tighter bases indicate stronger accumulation.",
  "mechanism": "Tight bases form when buyers absorb all selling pressure within a narrow range, creating a supply/demand imbalance at breakout.",
  "falsification_criteria": "Reject if profit_factor(treatment) <= profit_factor(control) * 1.10 OR total_trades(treatment) < 30",

  "type": "parameter_test",
  "asset_class": "crypto",
  "setup": "breakout",
  "date_range": {
    "start": "2017-01-01",
    "end": "2021-12-31",
    "split": "training"
  },

  "control_params": {
    "instrument_id": "BTCUSDT.BINANCE",
    "bar_type": "BTCUSDT.BINANCE-1-DAY-LAST-EXTERNAL",
    "rs_pct": 0.40,
    "vol_ratio": 1.5,
    "max_range_pct": 0.15,
    "max_dist_52w": 0.25,
    "min_adv": 0,
    "slippage_k": 0.10,
    "min_prior_move": 0.30,
    "max_sma_ext": 0.10,
    "regime": true,
    "risk_pct": 0.02,
    "max_pos_pct": 0.20,
    "split_frac": 0.50,
    "min_adr_pct": 0.03,
    "min_consol_days": 5,
    "min_price": 0,
    "min_vol": 0,
    "rs_lookback": 21,
    "max_hold_bars": 0
  },
  "treatment_params": {
    "max_range_pct": 0.10
  },

  "commands": [
    "python3 scripts/run_backtest.py --config control.json",
    "python3 scripts/run_backtest.py --config treatment.json"
  ],

  "control_results": {
    "total_trades": 208,
    "win_rate": 0.34,
    "profit_factor": 1.44,
    "sharpe": 1.12,
    "sortino": 1.67,
    "cagr": 0.056,
    "max_drawdown": 0.12,
    "avg_win": 523.50,
    "avg_loss": 212.30,
    "avg_hold_days": 8.3,
    "total_return": 0.281,
    "init_cash": 100000,
    "final_equity": 128100
  },
  "treatment_results": {
    "total_trades": 142,
    "win_rate": 0.31,
    "profit_factor": 1.52,
    "sharpe": 1.21,
    "sortino": 1.74,
    "cagr": 0.048,
    "max_drawdown": 0.10,
    "avg_win": 610.20,
    "avg_loss": 198.10,
    "avg_hold_days": 9.1,
    "total_return": 0.231,
    "init_cash": 100000,
    "final_equity": 123100
  },

  "improvement": {
    "primary_metric": "profit_factor",
    "control_value": 1.44,
    "treatment_value": 1.52,
    "pct_change": 5.6,
    "passes_threshold": false
  },

  "red_flags": [],

  "robustness_checks": {
    "sub_period_1_pf": null,
    "sub_period_2_pf": null,
    "sub_period_3_pf": null,
    "param_sensitivity_worst_sharpe_drop_pct": null,
    "cost_sensitivity_still_profitable": null
  },

  "walk_forward": {
    "wfe": null,
    "is_sharpe_avg": null,
    "oos_sharpe_avg": null,
    "num_windows": null
  },

  "conclusion": "Tighter consolidation range marginally improves profit factor (+5.6%) but reduces trade count by 32%. The improvement does not meet the pre-registered 10% threshold. REJECT.",
  "decision": "rejected",
  "next_steps": [
    "Test interaction: max_range_pct=0.10 combined with min_consol_days=7",
    "Run parameter sweep of max_range_pct from 0.05 to 0.30 to map the full response surface"
  ],

  "duration_seconds": 12.5,
  "notes": "NautilusTrader engine, 80 instruments loaded"
}
```

---

## Field Reference

### Identity Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Sequential ID, never reused. Format: `EXP-001`, `EXP-002`, ... |
| `date` | string | yes | ISO date of experiment (YYYY-MM-DD) |
| `status` | string | yes | `pending` \| `running` \| `rejected` \| `promoted` \| `needs_further_testing` |
| `parent_id` | string \| null | yes | ID of experiment this follows up on (`null` if first in chain) |
| `corrects_id` | string \| null | no | ID of a prior experiment this entry corrects (rare, for errors) |

### Hypothesis Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `hypothesis` | string | yes | The testable claim, stated before running the experiment |
| `mechanism` | string | yes | Why this should work — the behavioral or structural reason |
| `falsification_criteria` | string | yes | The specific, objective criteria that would reject this hypothesis |

### Experiment Design Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | yes | One of: `parameter_test`, `parameter_sweep`, `optimization`, `walk_forward`, `sub_period`, `cross_asset`, `regime_analysis`, `new_filter`, `validation`, `holdout` |
| `asset_class` | string | yes | `stocks` \| `crypto` |
| `setup` | string | yes | `breakout` \| `ep` \| `parabolic` \| `all` |
| `date_range` | object | yes | `{start, end, split}` where split = `training` \| `validation` \| `holdout` \| `walk_forward` |
| `control_params` | object | yes | Full parameter set for the baseline run |
| `treatment_params` | object | yes | Parameter set for the treatment (can omit unchanged params) |

### Reproduction Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `commands` | string[] | yes | Exact commands used, for reproducibility. Python scripts or notebook cells. |

### Result Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `control_results` | object | yes | All metrics from the control run |
| `treatment_results` | object | yes | All metrics from the treatment run |

Result object fields (extracted from NautilusTrader reports):
- `total_trades`, `win_rate`, `profit_factor`, `avg_win`, `avg_loss`
- `avg_hold_days`, `total_return`, `cagr`, `max_drawdown`
- `sharpe`, `sortino`, `init_cash`, `final_equity`

### Analysis Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `improvement` | object | when applicable | Primary metric comparison: `{primary_metric, control_value, treatment_value, pct_change, passes_threshold}` |
| `red_flags` | string[] | yes | List of triggered red flags (empty if none) |
| `robustness_checks` | object | when promoted | Sub-period PFs, sensitivity worst drop, cost sensitivity |
| `walk_forward` | object | when WFA done | `{wfe, is_sharpe_avg, oos_sharpe_avg, num_windows}` |

### Conclusion Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `conclusion` | string | yes | 2–3 sentence summary of findings |
| `decision` | string | yes | `rejected` \| `promoted_to_robustness` \| `promoted_to_optimization` \| `promoted_to_wfa` \| `promoted_to_validation` \| `promoted_to_holdout` \| `viable` |
| `next_steps` | string[] | yes | 1–3 items: what to test next (regardless of pass/fail) |

### Metadata Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `duration_seconds` | number | no | Wall-clock time for all backtests in this experiment |
| `notes` | string | no | Free-form notes |

---

## Conventions

- **IDs are sequential**: `EXP-001`, `EXP-002`, etc. To find the next ID, read the last line of `lab-notebook.jsonl`.
- **Append-only**: never modify existing entries. If you made an error, add a new entry with `corrects_id` pointing to the erroneous one.
- **Treatment params can be sparse**: only include parameters that differ from defaults.
- **Control params should be complete**: include all parameters for full reproducibility, even if they're defaults.
- **Commands must be reproducible**: someone should be able to reproduce the exact experiment by running the recorded commands (Python scripts with config files or inline params).

---

## Companion Files

- **`strategy-registry.json`** — Array of strategies that have reached Stage 5 (validation) or Stage 6 (holdout). Created on first promotion. Each entry includes exact params, all validation metrics, promotion date, and current status.

- **Session memory** — Key findings and parameter regions explored. Updated at end of each session. Used to avoid re-testing and to maintain research continuity.
