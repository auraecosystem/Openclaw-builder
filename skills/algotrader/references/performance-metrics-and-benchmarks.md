# Performance Metrics, Benchmarks, and Realistic Expectations

## Why Metrics Matter

Raw return numbers lie. A strategy that returned 50% last year might have taken on catastrophic risk to do it. A strategy returning 18% with near-zero drawdown might be exceptional. The metrics below are the professional toolkit for evaluating performance honestly.

---

## Core Metrics

### Sharpe Ratio

**Formula**:
```
Sharpe = (mean_annual_return − risk_free_rate) / annual_volatility
```

**Interpretation**:
| Sharpe | Assessment |
|--------|-----------|
| < 0.5 | Poor — barely better than holding cash |
| 0.5-1.0 | Acceptable — roughly in line with S&P 500 buy-and-hold |
| 1.0-1.5 | Good — meaningfully better risk-adjusted performance |
| 1.5-2.0 | Very good — professional standard for retail |
| 2.0-2.5 | Excellent — institutional quality |
| > 2.5 | Exceptional OR overfitted (investigate carefully) |
| > 3.5 | Almost certainly overfitted in backtesting |

**Reference points**:
- S&P 500 long-run: ~0.45-0.55 Sharpe
- Best mutual funds over 10+ years: ~0.6-0.8
- Good CTA funds: 0.8-1.2
- Top quant hedge funds: 1.5-2.5
- Renaissance Medallion: ~2.5-3.0 net (the best ever recorded over long periods)

**Limitations of Sharpe**:
- Assumes returns are normally distributed (they're not — fat tails, skew)
- Penalizes upside volatility equally with downside volatility
- Looks terrible for trend-following (long periods of small losses, then large gains)

### Sortino Ratio

Like Sharpe, but only penalizes **downside** volatility:

```
Sortino = (mean_annual_return − risk_free_rate) / downside_deviation
```

**Better for**: strategies with positive skew (right tail is good), trend following, options premium selling.

A strategy where most losses are small and rare gains are large will have a much better Sortino than Sharpe.

### Maximum Drawdown (MDD)

**The single most important risk metric for psychological sustainability.**

```
MDD = (Peak equity − Trough equity) / Peak equity
```

| MDD | Psychological Impact |
|-----|---------------------|
| < 10% | Comfortable; most traders can sustain |
| 10-20% | Manageable; requires pre-commitment to rules |
| 20-30% | Difficult; high risk of strategy abandonment |
| 30-50% | Extremely difficult; most retail traders quit here |
| > 50% | Nearly impossible to sustain psychologically |

**Critical**: even great strategies can draw down 2-3× their typical MDD during tail events. If your average drawdown is 12%, plan for a 30%+ tail event.

### Calmar Ratio

```
Calmar = Annual Return / Maximum Drawdown
```

Answers: "For each percent of risk I took (measured by max drawdown), how much return did I get?"

- Calmar > 1.0: decent (1% return per 1% drawdown)
- Calmar > 2.0: good
- Calmar > 3.0: excellent

### Profit Factor

```
Profit Factor = Total Gross Profits / Total Gross Losses
```

| Profit Factor | Assessment |
|--------------|-----------|
| < 1.0 | Net loser |
| 1.0-1.2 | Marginal — transaction costs likely eliminate edge |
| 1.2-1.5 | Viable minimum |
| 1.5-2.0 | Good |
| > 2.0 | Strong |
| > 3.0 | Investigate for overfitting |

### Win Rate and Payoff Ratio

These two metrics must be evaluated together:

```
Expectancy = (Win Rate × Avg Win) − (Loss Rate × Avg Loss)
```

A strategy with 35% win rate and 3:1 average win/loss ratio is profitable:
```
Expectancy = (0.35 × 3) − (0.65 × 1) = 1.05 − 0.65 = +0.40 per unit risked
```

A strategy with 65% win rate and 0.5:1 average win/loss ratio is a loser:
```
Expectancy = (0.65 × 0.5) − (0.35 × 1) = 0.325 − 0.35 = −0.025 per unit risked
```

**Common mistake**: evaluating win rate in isolation. High win rate feels good psychologically but says nothing about profitability.

### Recovery Factor

```
Recovery Factor = Net Profit / Maximum Drawdown
```

How many times greater is your profit than your worst loss? Higher is better.

---

## Evaluating Backtest Performance Honestly

### The Overfitting Discount

Before applying any performance number from a backtest to live expectations, apply these discounts:

| Factor | Typical Discount |
|--------|-----------------|
| Overfitting / curve fitting | 20-40% haircut to Sharpe |
| Transaction costs (if not modeled) | 30-60% haircut depending on turnover |
| Walk-forward efficiency (median) | 50-70% of in-sample Sharpe |
| Market impact at full size | 5-20% additional discount |

**Conservative live performance estimate**:
```
Expected live Sharpe = Backtest Sharpe × WFE ratio × (1 − cost drag fraction)

Example:
Backtest Sharpe: 2.0
WFE ratio: 0.60
Cost drag fraction: 0.25

Expected live Sharpe = 2.0 × 0.60 × 0.75 = 0.90
```

This is not pessimism — it is calibration. The strategy at 0.90 Sharpe live is still genuinely good. The mistake is deploying capital based on the 2.0 Sharpe and being shocked by reality.

### The Minimum Viable Strategy Thresholds

Before going live, a strategy must meet **all** of these after realistic cost adjustment:

| Metric | Minimum | Notes |
|--------|---------|-------|
| Sharpe Ratio | > 1.0 | Net of all costs |
| Maximum Drawdown | < 25% | Historical; tail event could be 2× |
| Profit Factor | > 1.3 | Enough margin above breakeven |
| Walk-Forward Efficiency | > 0.5 | Confirms it's not pure overfitting |
| Profitable sub-periods | ≥ 2 of 3 | Works across historical thirds |
| Monte Carlo (80th percentile) | Positive | Strategy is robust to ordering |

---

## Realistic Annual Return Expectations

This is where most retail traders have dangerously wrong expectations.

### What Sophisticated Funds Actually Achieve

| Entity | Net Annual Return | Sharpe (approx) |
|--------|-----------------|----------------|
| S&P 500 index (long-run) | 10% | 0.5 |
| Average mutual fund | 8-9% (before fees) | 0.4-0.5 |
| Top quant hedge funds (Two Sigma, AHL) | 15-25% | 0.8-1.5 |
| Renaissance Medallion (exceptional) | 39% net | ~2.5-3.0 |

### What Retail Algo Traders Should Target

| Experience Level | Realistic Annual Return | Notes |
|-----------------|------------------------|-------|
| Beginner (1-2 years) | Break-even to 10% | Most beginners lose money; breaking even is a win |
| Intermediate (3-5 years) | 10-20% | After proper validation and cost modeling |
| Advanced (5+ years) | 15-30% | Portfolio of strategies, risk-managed |
| Exceptional retail | 30-50% | Possible in niche illiquid spaces with structural advantage |

Anyone advertising 100%+ annual returns from an algo system is either:
(a) Running enormous risk you're not seeing
(b) In a very short lucky streak
(c) Lying

### The Returns-Risk Trade-off

Higher returns require either higher Sharpe (harder) or higher leverage/risk (accessible but dangerous).

Running 5:1 leverage on a Sharpe 0.8 strategy generates high nominal returns but also 5× the drawdowns. The strategy doesn't get better — you just amplify both the gains and the losses.

Professional quant funds often run moderate Sharpe strategies (0.8-1.5) with controlled leverage. The skill is in the risk management, not in generating outlandish raw returns.

---

## Monitoring Live Performance

### The Rolling Metrics Dashboard

Check these daily/weekly during the first 90 days of live trading:

```
Rolling 30-day metrics:
  - Annualized return (trailing 30 days)
  - Annualized volatility (trailing 30 days)
  - Rolling Sharpe (trailing 30 days)
  - Current drawdown from peak
  - Win rate (trailing 20 trades)
  - Average fill slippage vs. modeled

Rolling 60-day metrics:
  - Rolling Sharpe vs. expected
  - Profit factor (trailing 60 days)
  - Parameter stability check (if optimal params have shifted significantly)
```

### Distinguishing Noise from Signal

With a strategy making 100 trades/year at 55% win rate:
- Standard error of win rate estimate = √(0.55 × 0.45 / 100) = 5%
- 95% confidence interval: 45-65% win rate from observed data

After 100 trades, you still have enormous uncertainty about the strategy's true parameters. After 50 trades, even more so.

**Implication**: you need **statistical discipline** to avoid reacting to noise. A 3-week losing streak on a strategy making 2 trades/week is only 6 trades — statistically meaningless.

---

## Sources

- [Sharpe Ratio for Algorithmic Trading — QuantStart](https://www.quantstart.com/articles/Sharpe-Ratio-for-Algorithmic-Trading-Performance-Measurement/)
- [How to Evaluate a Trading Strategy Like a Quant — Medium / Yavuz Akbay](https://medium.com/@yavuzakbay/how-to-evaluate-a-trading-strategy-like-a-quant-fc903e093015)
- [Essential Backtesting Metrics — QuantStrategy.io](https://quantstrategy.io/blog/essential-backtesting-metrics-understanding-drawdown-sharpe/)
- [Quant Trading Performance 101 — Medium / Irene Aldridge](https://medium.com/@irenealdridge/quant-trading-and-hft-performance-101-bc3ef47bceef)
- [Realistic Expectations in Algo Trading — QuantConnect Forum](https://www.quantconnect.com/forum/discussion/5720/realistic-expectations-in-algo-trading/)
- [One Year of Full-Time Quant Trading — Medium / QFX Research](https://medium.com/qfx-research/im-approaching-one-year-since-diving-full-time-into-quant-trading-a1bea7bb6d05)
- [The Medallion Fund: Greatest Money-Making Machine — Marcellus](https://marcellus.in/story/why-the-medallion-fund-is-the-greatest-money-making-machine-of-all-time/)
