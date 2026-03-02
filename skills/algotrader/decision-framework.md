# Decision Framework — Promotion Ladder & Thresholds

## The Promotion Ladder

Each strategy progresses through 7 stages. **Demotion is permanent** — no going back to re-optimize after seeing later-stage results.

```
Stage 0: HYPOTHESIS
    ↓ (plausible mechanism, falsifiable, testable)
Stage 1: INITIAL SCREEN (training set)
    ↓ (PF > 1.0, trades >= 30, no hard stops)
Stage 2: ROBUSTNESS (sub-period, sensitivity, costs)
    ↓ (2/3 sub-periods profitable, params stable ±20%, profitable at slippage 0.15)
Stage 3: OPTIMIZATION (evolve.py, max 5 params)
    ↓ (Sharpe < 3.0, sensitivity passes, diversity > 0.05)
Stage 4: WALK-FORWARD VALIDATION (rolling IS/OOS)
    ↓ (WFE > 0.50)
Stage 5: VALIDATION SET (2016–2020)
    ↓ (Sharpe > 0.5 × training Sharpe, PF > 1.2, MDD < 30%)
Stage 6: HOLDOUT (2021–2024) — ONE SHOT
    ↓ (Sharpe > 1.0, PF > 1.3, MDD < 25%, trades >= 30)
    ✓ VIABLE — Ready for paper trading
```

---

## Stage Details

### Stage 0: Hypothesis

**Passes if**:
- Plausible mechanism exists (can explain why someone is on the other side of the trade)
- Hypothesis is falsifiable (specific metric threshold defined)
- Testable with current infrastructure

**Fails if**:
- No mechanism ("I just think it'll work")
- Untestable without major code changes
- Already tested (check lab notebook)

**Action on pass**: write lab notebook header, proceed to Stage 1.

---

### Stage 1: Initial Screen

Run on training set (start–2021-12-31 for crypto, 1997–2015 for stocks) with proposed parameters.

**Passes if**:
- Profit factor > 1.0
- Total trades >= 30
- No hard-stop red flags triggered

**Fails if**:
- PF <= 1.0 (not profitable before any optimization)
- Total trades < 10
- Any hard stop triggered (see `guardrails.md`)

**Action on pass**: promote to Stage 2.
**Action on fail**: **REJECT**. Record in notebook. Move to next hypothesis.

---

### Stage 2: Robustness

Run sub-period analysis, parameter sensitivity, and cost sensitivity.

**Passes if**:
- PF > 1.0 in at least 2 of 3 sub-periods (crypto: pre-2019/2019-2020/2020-2021; stocks: 1997–2003/2004–2009/2010–2015)
- No optimized parameter's ±20% perturbation drops Sharpe > 30%
- Strategy remains profitable (PF > 1.0) at `slippage_k = 0.15`

**Fails if**:
- Any robustness check fails

**Action on pass**: promote to Stage 3.
**Action on fail**: **REJECT** or **INVESTIGATE** (if close to thresholds, try simplifying the strategy — fewer params, wider filter ranges).

---

### Stage 3: Optimization

Run grid search or parameter sweep on training set, maximum 5 free parameters.

**Passes if**:
- Best params have Sharpe < 3.0
- Top-10 results show diversity (not all clustered at identical params)
- Immediate sensitivity test passes (±20% on each optimized param, Sharpe drop < 30%)

**Fails if**:
- Sharpe > 3.5 (hard stop)
- Sensitivity test fails
- All top results converge to a narrow peak (overfit risk)

**Action on pass**: promote to Stage 4.
**Action on fail**: **REJECT** or reduce to fewer parameters and retry Stage 3 once.

---

### Stage 4: Walk-Forward Validation

Run rolling walk-forward analysis on training set (crypto: 12m IS / 3m OOS; stocks: 24m IS / 6m OOS).

**Passes if**: WFE > 0.50
**Marginal**: WFE 0.30–0.50 → simplify strategy (fewer params) and retry once
**Fails if**: WFE < 0.30

**Action on pass**: promote to Stage 5.
**Action on fail**: **REJECT**.

---

### Stage 5: Validation Set

Run on validation set (crypto: 2022-01-01 to 2023-06-30; stocks: 2016-01-01 to 2020-12-31) with optimized params from WFA.

**Passes if**:
- Sharpe > 0.5 × average training Sharpe (from WFA in-sample windows)
- Profit factor > 1.2
- Max drawdown < 30%
- Total trades >= 20

**Fails if**:
- Any validation metric below threshold

**Action on pass**: promote to Stage 6. Add to `strategy-registry.json`.
**Action on fail**: **REJECT**. Do NOT re-optimize. Do NOT touch holdout. Strategy is dead.

---

### Stage 6: Holdout — Final Verdict

Run on holdout set (crypto: 2023-07-01 to present; stocks: 2021-01-01 to 2024-12-31). **ONE SHOT. NO RETRIES.**

**Passes if ALL**:
- Sharpe > 1.0 (net of all modeled costs)
- Profit factor > 1.3
- Max drawdown < 25%
- Total trades >= 30
- Walk-forward efficiency > 0.50 (from Stage 4, not re-tested)
- Sub-period profitable in >= 2 of 3 periods (from Stage 2, not re-tested)
- The mechanism is explainable in plain English

**Fails if**: any metric below threshold.

**Action on pass**: status = **VIABLE**. Document fully in strategy registry. Ready for paper trading.
**Action on fail**: **REJECT PERMANENTLY**. No modifications, no re-testing. Start over with a new hypothesis.

---

## Expected Rejection Rates

Based on the reference materials and realistic expectations:

| Transition | Rejection Rate | Survivors |
|-----------|---------------|-----------|
| Stage 0 → 1 | ~70% | 30 of 100 hypotheses |
| Stage 1 → 2 | ~50% | 15 |
| Stage 2 → 3 | ~40% | 9 |
| Stage 3 → 4 | ~30% | 6 |
| Stage 4 → 5 | ~40% | 4 |
| Stage 5 → 6 | ~30% | 3 |
| Stage 6 → Viable | ~30% | **~2** |

**Net: roughly 1–2 out of 100 hypotheses should result in a viable strategy.** This is normal. If your hit rate is significantly higher, you are likely not being rigorous enough.

---

## Minimum Viable Strategy Thresholds (Summary)

All must be met simultaneously on the holdout set:

| Metric | Minimum | Notes |
|--------|---------|-------|
| Sharpe Ratio | > 1.0 | Net of all modeled costs |
| Max Drawdown | < 25% | Tail events could be 2× worse in live trading |
| Profit Factor | > 1.3 | Enough margin above breakeven |
| Total Trades | >= 30 | Statistical significance |
| Walk-Forward Efficiency | > 0.50 | Confirmed at Stage 4 |
| Sub-Period Profitability | 2/3 profitable | Confirmed at Stage 2 |
| Mechanism | Explainable | Can state why the counterparty persistently loses |

---

## What "Viable" Means (and Doesn't Mean)

### Viable DOES mean:
- The strategy survived all adversarial tests in the research process
- It is ready for **paper trading** (30+ days, live market conditions)
- It has earned the right to be tested with real market data in real time

### Viable DOES NOT mean:
- "Guaranteed to make money"
- "Ready for real capital deployment"
- "Will maintain this performance indefinitely"

### After Viable:
1. **Paper trade** for 30+ days in live market conditions
2. Compare paper results to backtest expectations (within normal variance)
3. Only after paper trading confirms metrics: consider real capital at 10–25% of intended size
4. Scale to full size only after 60–90 days of live results matching expectations

---

## Memory System: Building Institutional Knowledge

The agent accumulates knowledge across sessions through three files:

### 1. Lab Notebook (`lab-notebook.jsonl`)
- Every experiment, including failures
- Read at session start; the `next_steps` fields form a research queue
- Append-only; never modify

### 2. Strategy Registry (`strategy-registry.json`)
- Strategies that have reached Stage 5 or Stage 6
- Each entry: exact params, all validation metrics, promotion date, current status
- Created on first promotion; updated in-place

### 3. Session Memory
- Key findings and patterns discovered
- Parameter regions explored (to avoid re-testing)
- Failed hypothesis families (to avoid re-testing)
- Infrastructure notes (engine quirks, data issues)
- Updated at end of every session

### Session Start Protocol
1. Read all three files
2. Identify the research frontier (last experiment's `next_steps`)
3. Check for any strategies in the pipeline that need advancement
4. Begin Phase 1 of the playbook

### Session End Protocol
1. Ensure all experiments are recorded in the lab notebook
2. Update session memory with key findings
3. Update strategy registry if any promotions occurred
4. Verify the last notebook entry has clear `next_steps`
