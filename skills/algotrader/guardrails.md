# Guardrails and Red Flags

## Hard Stops — Immediately Reject

If any of these are true, reject the strategy without further investigation:

- [ ] Backtest Sharpe > 3.5 on any single test period
- [ ] Total trades < 10 in any test period
- [ ] Strategy requires > 10 tunable parameters to be profitable
- [ ] Holdout set was used during development (data contamination — entire research line is invalidated)
- [ ] Walk-forward efficiency < 0.30
- [ ] Profit factor < 1.0 after realistic cost modeling (`slippage_k` >= 0.10)
- [ ] Strategy only works on one specific ticker or small group (<5) of tickers
- [ ] No plausible mechanism — cannot explain why the other side of the trade persistently loses money

---

## Yellow Flags — Investigate Before Proceeding

These don't automatically reject but require extra scrutiny:

- [ ] Sharpe 2.5–3.5 (possible overfitting; run extra robustness checks)
- [ ] Total trades 10–30 (low statistical power; results are noisy)
- [ ] Parameters are suspiciously precise (e.g., `rs_pct=0.0347` — real edges don't live at such specific values)
- [ ] Performance concentrated in one sub-period (e.g., only 2010–2015)
- [ ] Performance degrades > 30% when perturbing any optimized parameter ±20%
- [ ] Win rate > 60% for a momentum strategy (unusual — verify carefully)
- [ ] Max drawdown < 5% over a multi-year period (suspiciously smooth — may indicate too few trades or look-ahead)
- [ ] Strategy works much better on training than validation (> 40% Sharpe degradation)
- [ ] Evolutionary optimizer converged to low diversity (< 0.05) early — entire population found a narrow peak

---

## Overfitting Indicators — Ranked by Severity

| Severity | Indicator |
|----------|-----------|
| **CRITICAL** | Optimization used more than 7 free parameters |
| **CRITICAL** | WFE < 0.30 (pure curve fit — the strategy explains past noise, not future signal) |
| **CRITICAL** | Strategy fails on 2+ of 3 sub-periods of training data |
| **HIGH** | Parameter sensitivity: performance cliff when any param moves ±20% |
| **HIGH** | Sharpe > 3.0 on training data (very few strategies genuinely achieve this) |
| **MEDIUM** | Trade count < 50 total across all test periods |
| **MEDIUM** | Strategy does not transfer to related asset class at all (zero overlap in behavior) |
| **LOW** | Optimal parameters are at the boundary of the search range (true optimum may be outside tested range) |
| **LOW** | Strategy performance is highly sensitive to exact start/end dates of the test period |

---

## Data Snooping Prevention

### Before Each Experiment

Ask yourself: **"Have I already used this data to make decisions?"**

If you ran the engine on a specific date range and used the result to choose your next experiment, that date range is no longer fully out-of-sample. It has become part of your implicit optimization loop.

### Implicit Multiple Comparisons

If you ran 10 parameter sweeps on the same training data, you have effectively used `10 × parameter_count` degrees of freedom. Apply implicit Bonferroni: your significance threshold should be stricter (require >15-20% improvement rather than 10%).

### Validation Set Integrity

The validation set (2016–2020) can be used **once** for final pre-holdout tuning. If you "peeked" at validation results and then changed your strategy, you have contaminated the validation set. Be honest about this — note it in the lab notebook.

### Holdout is One Shot

The holdout set (2021–2024) is touched **exactly once**. No exceptions. No "let me just check one thing." Once you see holdout results, the experiment is over and the verdict is final.

If you violate this rule, note it in the lab notebook and acknowledge that the holdout result is no longer trustworthy. You effectively no longer have a valid holdout set.

---

## Transaction Cost Sanity Check

Before declaring any strategy viable, run it at three cost levels:

| Level | `slippage_k` | Purpose |
|-------|-------------|---------|
| Baseline | 0.10 | Standard assumption |
| Conservative | 0.15 | Accounts for above-average slippage |
| Pessimistic | 0.20 | Stress test for cost sensitivity |

**Rules**:
- If unprofitable at `slippage_k = 0.15`: the strategy is **fragile**. Real-world slippage for momentum strategies is typically 0.10–0.20.
- If unprofitable at `slippage_k = 0.10`: the strategy has no meaningful edge.
- For crypto: add an extra ~0.10% to account for exchange taker fees (typically 0.075–0.10% on major exchanges).

### Breakeven Slippage

Calculate the `slippage_k` at which the strategy breaks even (PF = 1.0). Record this in the lab notebook. It tells you how much execution quality margin you have.

---

## Sample Size Guidelines

Minimum trade counts for statistical significance, by holding period:

| Holding Period | Min Trades / Year of Data | Min Total Trades |
|---------------|--------------------------|-----------------|
| Intraday | 500+ | 2000+ |
| 1–5 days | 100+ | 500+ |
| 5–20 days (typical for algotrader setups) | 30+ | 200+ |
| 20+ days | 15+ | 100+ |

### Statistical Power

The ability to detect real Sharpe ratio differences depends on sample size:

| Total Trades | Detectable Sharpe Difference | Confidence |
|-------------|------------------------------|------------|
| 50 | ~0.8 | Very coarse — can only detect very large effects |
| 100 | ~0.5 | Coarse — may miss moderate improvements |
| 500 | ~0.2 | Reasonable — can detect meaningful differences |
| 1000+ | ~0.1 | Good — fine-grained comparison possible |

**Practical implication for crypto**: with the typical 40–100 trades/year from the breakout setups on the ~5-year crypto training set, you have ~200–500 total trades. This gives reasonable power to detect Sharpe differences of ~0.3. Improvements smaller than that cannot be reliably distinguished from noise.

**Practical implication for stocks**: with the typical 30–100 trades/year on the 19-year training set, you have ~600–1,900 total trades. This gives reasonable power to detect Sharpe differences of ~0.2.

---

## The "Too Good to Be True" Heuristic

If a result seems too good, it almost certainly is. Apply extra skepticism when:

- A simple one-parameter change doubles the Sharpe ratio
- An optimization finds a strategy with zero losing months
- A strategy works perfectly on all three sub-periods with no degradation
- The equity curve is monotonically increasing with tiny drawdowns
- The strategy has a higher Sharpe than Renaissance Technologies' Medallion Fund (~2.5–3.0 net)

The correct response is not excitement — it is suspicion. Investigate the mechanism. Check for look-ahead bias. Check for survivorship bias. Check whether the result depends on a handful of trades that may be data errors.
