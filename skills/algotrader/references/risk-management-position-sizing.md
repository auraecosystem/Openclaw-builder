# Risk Management and Position Sizing

## The Hierarchy

**Risk management is the first priority. Returns are the last.**

83% of successful algo traders cite risk management as their #1 concern — above signal quality, above returns. This is not modesty. It reflects a mathematical reality: a strategy with a Sharpe of 1.0 and excellent drawdown control will, over time, outperform a Sharpe 2.0 strategy that blows up accounts.

You cannot make money from a blown account. Survival is the prerequisite for everything else.

---

## The Kelly Criterion

Developed by John Kelly Jr. at Bell Labs (1956). Calculates the fraction of capital to bet per trade to **maximize long-term geometric growth**.

### Formula

```
Kelly % = (W × R − L) / R

Where:
  W = win rate (decimal, e.g. 0.55 for 55%)
  L = loss rate = 1 − W
  R = reward/risk ratio = average win / average loss
```

### Example

Strategy with 55% win rate and 1.5:1 reward/risk ratio:
```
Kelly = (0.55 × 1.5 − 0.45) / 1.5
      = (0.825 − 0.45) / 1.5
      = 0.375 / 1.5
      = 25% of capital per trade
```

### Why You Should NOT Use Full Kelly

Full Kelly is mathematically optimal for long-run wealth maximization. In practice it is psychologically and practically catastrophic:

- **Drawdowns are extreme**: Full Kelly experiences 50% drawdowns regularly
- **Estimation error**: Your actual W and R will differ from historical; Kelly is extremely sensitive to these inputs
- **Sequence dependence**: A bad run early (before the law of large numbers kicks in) can wipe out accounts at full Kelly

**Professional practice**: use **Half Kelly (0.5×)** or **Quarter Kelly (0.25×)**. This sacrifices roughly 25% of expected terminal wealth growth in exchange for dramatically lower volatility and drawdowns.

### When Kelly Breaks Down

- Kelly assumes i.i.d. (independent, identically distributed) trades — real markets have autocorrelation
- Kelly assumes you know W and R precisely — you don't; use confidence intervals
- Kelly doesn't account for correlation between simultaneous open positions

---

## Volatility Targeting (Robert Carver / AHL Framework)

This is the professional systematic trading standard, used by Man AHL, Winton, and most CTA funds. The core principle: **size every position so it contributes a fixed, equal amount of portfolio volatility**.

### Why Volatility Targeting

Markets change their volatility over time. A position sized for "normal" conditions becomes dramatically over- or under-sized when volatility regimes change. Volatility targeting automatically adjusts.

During the 2020 COVID crash, equity volatility increased ~5x. A volatility-targeted position would have automatically reduced to ~20% of its normal size, limiting losses.

### The Formula

```
position_size = (target_annual_vol × account_size) / (instrument_vol × price × point_value)

Where:
  target_annual_vol = your desired portfolio volatility (e.g. 0.20 = 20% annual)
  instrument_vol = instrument's recent annualized volatility (e.g. ATR-based)
  price = current price of the instrument
  point_value = contract multiplier (for futures)
```

### Practical Example

- Account: $100,000
- Target annual volatility: 20%
- Stock price: $50
- Stock's annualized volatility: 30%

```
shares = (0.20 × 100,000) / (0.30 × 50) = 20,000 / 15 = 1,333 shares
position value = 1,333 × $50 = $66,650
```

As the stock becomes more volatile (say 40%), position automatically shrinks to 1,000 shares. During low-vol periods, it expands. This is **automatic risk control without manual intervention**.

### Volatility Estimation

Use a measure that responds to regime changes quickly but isn't too noisy:
- **Exponential weighted volatility** (fast-responding): half-life of 10-25 days
- **ATR-based volatility**: Average True Range annualized
- Avoid simple historical volatility over long windows — too slow to react to regime changes

---

## Drawdown Management

### Pre-Trade Drawdown Rules (Set Before Going Live)

Write these down before deployment. Do not change them during a drawdown.

| Rule | Typical Threshold | Action |
|------|-----------------|--------|
| Daily loss limit | 2-3% of account | Stop trading for the day |
| Weekly loss limit | 5-7% of account | Stop trading for the week, review |
| Strategy drawdown | 15-25% from peak | Pause strategy, investigate whether structural or normal |
| Account drawdown | 20-30% from peak | Full stop, comprehensive review |

### Distinguishing Normal Drawdown from Strategy Failure

A critical skill: most strategies *will* have drawdowns. Abandoning a strategy during a normal drawdown (then watching it recover without you) is one of the most common and expensive retail mistakes.

Calculate **maximum historical drawdown** during backtesting/WFA. A drawdown within 1.5-2x the historical maximum is probably normal. A drawdown exceeding 2-3x historical maximum warrants serious investigation.

Statistical approach: if your strategy has expected Sharpe 1.0, you should expect a 30-day losing streak with probability ~7% in any given year. This is normal, not a sign of failure.

### When to Retire a Strategy

- Rolling 60-day Sharpe drops below 0.3 for 3+ consecutive months
- Drawdown exceeds 2.5x historical maximum drawdown
- Market structure clearly changed (e.g., the instrument the strategy traded ceased to exist or dramatically changed character)
- Multiple correlated strategies all fail simultaneously (suggests regime change, not strategy failure)

---

## Position Sizing in a Multi-Strategy Portfolio

When running multiple simultaneous strategies:

### Correlation Adjustment

If two strategies are correlated (both long equities, both momentum), their combined risk is **greater** than the sum of their individual risks. You must reduce individual position sizes to account for this.

```
portfolio_vol = sqrt(w1² × σ1² + w2² × σ2² + 2 × w1 × w2 × σ1 × σ2 × ρ)
```

Where ρ is the correlation coefficient between strategies. Even moderate correlation (ρ = 0.5) means two "half Kelly" positions together behave like a single position larger than either.

### Risk Parity

Alternative to Kelly: allocate capital so each strategy contributes **equal risk** (equal volatility contribution) to the portfolio. This means lower-volatility strategies get more capital; higher-volatility strategies get less.

Result: no single strategy dominates the portfolio's risk profile. The portfolio volatility is well-controlled even when individual strategies behave unexpectedly.

### Maximum Concentration Limits

Even if Kelly says to put 40% in one trade, apply caps:
- Single position: max 5-10% of account
- Single sector: max 20-25%
- Correlated group of strategies: max 30-40% of total risk budget

---

## Value at Risk (VaR) and Conditional VaR

### VaR
"With 95% confidence, I will not lose more than X% in any given day."

Calculated as: mean daily return − (1.645 × daily volatility)

**Limitation**: VaR tells you nothing about *how bad* the worst 5% of days could be. It only tells you where the 95th percentile boundary is.

### Conditional VaR (CVaR / Expected Shortfall)
"Given that I've exceeded my VaR threshold, what is the average loss?"

This is more useful for tail risk management. CVaR captures the severity of extreme events, not just their probability.

### Practical Use
- Monitor VaR daily; if it drifts up (e.g., strategy is in more volatile assets), reduce position size
- Use CVaR to size the "emergency fund" — the amount you must be willing to lose in a 1-in-20 day event

---

## The Psychological Contract with Risk Rules

**The most important risk management principle: risk rules are set before you have skin in the game.**

When you're in a losing trade, every cognitive bias works against you:
- Loss aversion makes the loss feel 2x worse than an equivalent gain
- Sunk cost fallacy makes you want to "wait for recovery"
- Overconfidence makes you believe "this time is different"

Pre-committing to rules — in writing, in code — removes the human element from the worst possible moment. Your risk rules should be **hard-coded into your system**, not suggestions you evaluate in real-time.

---

## Sources

- [The Kelly Criterion — Frontiers in Applied Mathematics](https://www.frontiersin.org/journals/applied-mathematics-and-statistics/articles/10.3389/fams.2020.577050/full)
- [Risk-Constrained Kelly Criterion — QuantInsti](https://blog.quantinsti.com/risk-constrained-kelly-criterion/)
- [Position Sizing Strategies for Algo Traders — Medium / Jakub Polec](https://medium.com/@jpolec_72972/position-sizing-strategies-for-algo-traders-a-comprehensive-guide-c9a8fc2443c8)
- [Systematic Trading — Robert Carver (Wiley)](https://www.amazon.com/Systematic-Trading-designing-trading-investing/dp/0857194453)
- [Sharpe Ratio for Algorithmic Trading — QuantStart](https://www.quantstart.com/articles/Sharpe-Ratio-for-Algorithmic-Trading-Performance-Measurement/)
- [Uncorrelated Assets and Strategies — QuantifiedStrategies](https://www.quantifiedstrategies.com/uncorrelated-assets-and-strategies/)
