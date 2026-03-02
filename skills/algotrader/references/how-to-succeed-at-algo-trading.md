# How to Succeed at Algo Trading

Synthesized from credible primary sources: QuantStart, Ernest Chan (Cornell PhD, QTS Capital), Robert Carver (AHL/Man Group), QuantifiedStrategies, BuildAlpha, QuantPedia, academic literature, and verified institutional patterns.

---

## The Core Mental Model

**Markets are a signal extraction problem under adversarial conditions.**

Your backtest is your hypothesis. Live trading is the experiment. Most strategies fail because the hypothesis was tested on the same data used to generate it, and the adversarial conditions (costs, slippage, regime change, crowding) were not modeled.

The scientific method — not financial intuition — is the correct operating framework. Treat every strategy like a physics experiment: state a falsifiable hypothesis, test it on unseen data, reject it if it fails.

---

## Phase 1: Finding an Edge

### Where Edges Actually Come From

| Source | Description | Durability |
|--------|-------------|------------|
| **Academic factor research** | Peer-reviewed anomalies (momentum, value, mean reversion, size) | Medium — decays post-publication but slowly |
| **Market microstructure** | Order flow imbalance, bid-ask dynamics, liquidity patterns | Short — but continually refreshes |
| **Regime-dependent behavior** | Different regimes (trend/mean-revert) call for different strategies | Medium — regime persistence varies |
| **Structural market quirks** | Illiquid instruments, exotic pairs, microcaps institutions can't touch | Durable — institutions physically can't compete |
| **Behavioral biases** | Overcorrection, panic selling, earnings drift | Medium — crowding erodes it |

**The retail structural advantage**: Institutions cannot enter microcaps, exotic forex, or thin instruments without moving the market. This is the single most durable edge available to retail algo traders — it requires no intelligence competition with RenTec.

### Where NOT to Look

- Patterns that only appeared in 1-2 years of data
- Strategies that require >10 parameters
- Anything with a backtest Sharpe >3.5 on the first pass (almost certainly overfit)
- Ideas copied from social media without independent validation

### Academic Research as Idea Source

QuantPedia catalogs ~700 peer-reviewed strategies. Important caveat: **post-publication, anomaly returns drop 26-58%** as crowding occurs. Use academic research as an idea generator, then test on your own universe with your own implementation. The concept is the spark; the implementation is the edge.

---

## Phase 2: Rigorous Validation (The Hard Part)

This is where 90% of strategies are correctly rejected. Treat failures as information, not failure.

### The Seven Deadly Sins of Backtesting

1. **Survivorship bias** — using only stocks still alive today. Overstates annual returns by 1-4%. Fix: use point-in-time databases (Compustat, CRSP, Norgate).
2. **Look-ahead bias** — using data unavailable at decision time. Example: using today's close to trigger today's open order. Fix: shift signals by 1 bar minimum; use release dates for fundamentals.
3. **Overfitting / curve fitting** — tuning parameters until the equity curve is smooth. Fix: use fewer parameters; test on held-out data you never touched.
4. **Data snooping (p-hacking)** — running 100 variations and reporting the best. Fix: Bonferroni correction; pre-register hypothesis before testing.
5. **Transaction cost underestimation** — ignoring commissions, spread, and market impact. Fix: model realistic costs (see Phase 4).
6. **Period selection bias** — only testing on favorable regimes. Fix: include crashes, sideways markets, rate-hike cycles.
7. **Incomplete data quality** — OHLC daily data misses intraday behavior and assumes mid-price fills. Fix: tick data or realistic fill assumptions.

### The Correct Testing Protocol

```
1. In-sample (60-70%): develop and optimize strategy
2. Validation (15-20%): tune parameters — but only one pass
3. Out-of-sample holdout (15-20%): touch ONCE, final verdict
```

Prefer **walk-forward analysis** over single-split:
- Optimize on rolling window (e.g., 12 months)
- Test on next window (e.g., 3 months)
- Roll forward and repeat
- Final out-of-sample efficiency ratio should be >50% of in-sample performance

**Monte Carlo simulation** (1,000+ reshuffled trade sequences) answers: "Was this just lucky ordering?" A robust strategy maintains positive expectancy across permutations.

### Robustness Checks (BuildAlpha framework)

- **Perturb parameters by ±20%**: if returns collapse, the strategy is fragile
- **Test across multiple instruments** in the same asset class
- **Test across multiple regimes**: bull, bear, sideways, high-vol, low-vol
- **Stress test on crisis periods**: 2000, 2008, 2020, 2022

---

## Phase 3: Risk Management and Position Sizing

**83% of successful algo traders cite risk management as their #1 priority — above returns.**

### The Kelly Criterion

Optimal fraction to bet per trade:

```
Kelly % = (W × R − L) / R
W = win rate, L = 1 − W, R = avg win / avg loss
```

**In practice, use Half Kelly or Quarter Kelly.** Full Kelly produces extreme drawdowns that most traders cannot psychologically sustain, leading to abandonment at the worst time.

### Volatility Targeting (Carver's Framework)

Robert Carver (formerly Man AHL): size every position so its *volatility contribution* is constant. As an instrument's volatility rises, reduce position size proportionally. This prevents low-volatility periods from creating false overconfidence and high-volatility periods from blowing up accounts.

```
position_size = (target_annual_vol × account_size) / (instrument_vol × price)
```

### Drawdown Rules

Set hard rules before going live:
- Max daily loss limit: stop trading for the day at X%
- Max strategy drawdown: pause/retire strategy at Y% from peak
- Correlation monitor: if two "uncorrelated" strategies suddenly correlate (common in crises), reduce total exposure

### Portfolio of Uncorrelated Strategies

This is Ray Dalio's "Holy Grail" and the professional desk standard: **run multiple uncorrelated strategies simultaneously**.

Adding 7 strategies with only 10% inter-correlation can *halve* your total risk with the same expected return. Correlation coefficient between strategies should be <0.3 ideally. Diversify across:
- Timeframes (intraday, daily, weekly)
- Asset classes (equities, futures, forex, crypto)
- Strategy types (trend-following, mean reversion, breakout)
- Regimes (some strategies designed to work when others fail)

---

## Phase 4: Transaction Costs and Market Reality

**The most common reason backtests don't translate to live performance.**

### Cost Components

| Component | Impact |
|-----------|--------|
| Commission | Fixed per trade; easy to model |
| Bid-ask spread | Paid on every entry and exit |
| Slippage | Fill worse than expected; especially on momentum (chasing moves) |
| Market impact | Your large order moves the price against you |
| Financing costs | Overnight margin, borrow costs for shorts |

**Momentum strategies suffer most from slippage** — you're buying something already moving, so fills naturally occur at worse prices. Mean reversion strategies suffer less because you trade against momentum.

### Modeling Costs

- For daily strategies: add 0.1-0.2% round-trip minimum
- For intraday: model spread directly; may exceed strategy alpha on high-frequency signals
- For thin instruments: market impact is non-linear (quadratic models more accurate than linear)

**Knight Capital lost $440M in 45 minutes (2012) deploying an untested algorithm.** Paper trading through live market conditions is mandatory before capital deployment.

---

## Phase 5: Strategy Decay and Lifecycle Management

### How Long Does an Edge Last?

| Strategy Type | Typical Lifespan |
|--------------|-----------------|
| HFT / microstructure | Days to weeks |
| Intraday momentum | 3-6 months |
| Swing / daily systems | 6-18 months |
| Macro / factor-based | 1-3+ years |

**Why edges decay**: When a profitable pattern is discovered, more capital flows in, competition increases, and the inefficiency gets arbitraged away. Crowded strategies see the first movers take most of the profit; latecomers get noise.

### Managing the Lifecycle

- Monitor live Sharpe ratio on a rolling 60-day window
- Set a "pause threshold" — if Sharpe drops below 0.5, suspend and re-evaluate
- Distinguish between: normal drawdown within strategy parameters vs. structural decay
- Have a pipeline of validated strategies waiting to deploy
- Former Two Sigma quants report deploying hundreds of small models, deprecating each as it decays

---

## Phase 6: Infrastructure and Execution

### The Minimum Stack

1. **Clean data**: survivorship-bias-free historical data (Norgate, Compustat, Polygon.io). This is non-negotiable — garbage in, garbage out.
2. **Backtesting engine**: event-driven (not vectorized) for realism. QuantConnect (cloud), Zipline, QSTrader, or custom.
3. **Walk-forward + Monte Carlo validation**: built into the workflow, not optional.
4. **Risk rules**: hard-coded stops, not suggestions.
5. **Broker API**: Interactive Brokers (IBKR) for retail; Rithmic for futures.
6. **VPS**: colocated near exchange. For non-HFT daily strategies, latency is secondary to reliability and uptime.
7. **Monitoring**: immutable timestamped logs, latency histograms per API hop, fill forensics.

### Paper Trading Protocol

Minimum 30 days of paper trading in live market conditions (not replay) before deploying capital. Check:
- Do fills match expectations?
- Does the strategy behave differently in high-volatility sessions?
- Are there API errors, partial fills, connection drops?

### Going Live

Start at 10-25% of intended capital. Scale up only after 60-90 days of live results matching paper trading expectations within normal variance.

---

## Phase 7: The Psychological Layer

This is underweighted in technical literature but widely cited by experienced practitioners.

**Beyond average intelligence, emotional makeup matters more.** Many highly intelligent people are poor traders because they rationalize rule-breaking in the moment.

| Failure Mode | Description |
|-------------|-------------|
| Override the system | "Just this once" manual intervention; usually wrong |
| Strategy abandonment at drawdown bottom | Quitting exactly when mean reversion would recover |
| Optimization loop | Endlessly tweaking after each loss, creating new overfitting |
| Overconfidence after wins | Increasing size after a run — before edge is confirmed |
| Anchoring to backtest | Refusing to retire a strategy because "it worked in testing" |

**The fix**: pre-commit to rules in writing before going live. Define "what does a normal drawdown look like?" and "what is the failure condition that triggers retirement?" before you have skin in the game. Carver's argument: long-term success comes from *consistency and rule adherence* more than predictive accuracy.

---

## Realistic Performance Benchmarks

| Metric | Minimum Viable | Good | Exceptional |
|--------|---------------|------|-------------|
| Sharpe Ratio | 1.0 | 1.5-2.0 | >2.5 |
| Max Drawdown | <30% | <20% | <10% |
| Profit Factor | >1.2 | >1.5 | >2.0 |
| Win Rate | Strategy-dependent (low WR with high R:R is fine) | | |

For reference: S&P 500 buy-and-hold achieves ~0.5 Sharpe. A retail strategy at Sharpe 1.5 is genuinely strong. Sharpe >3.5 in backtesting is almost always overfitting.

---

## The Hierarchy of What Matters

1. **Not losing money** (risk rules, position sizing, drawdown limits)
2. **Not fooling yourself** (rigorous validation, no look-ahead, no overfitting)
3. **Realistic cost modeling** (slippage, spread, market impact)
4. **Strategy durability** (uncorrelated portfolio, lifecycle management)
5. **Returns** (last on the list — a consequence of getting 1-4 right)

---

## Key Books (Canonical References)

| Book | Author | Core Contribution |
|------|--------|------------------|
| *Quantitative Trading* (2nd ed.) | Ernest Chan | Full workflow from idea to live trading; realistic expectations |
| *Algorithmic Trading* | Ernest Chan | Mean reversion + momentum strategies with code |
| *Systematic Trading* | Robert Carver | Position sizing, volatility targeting, forecast diversification |
| *Advances in Financial ML* | Marcos Lopez de Prado | Avoiding backtest fraud; feature engineering; meta-labeling |
| *Successful Algorithmic Trading* | Michael Halls-Moore (QuantStart) | Practical Python implementation guide |

---

## Sources

- [Successful Backtesting of Algorithmic Trading Strategies — QuantStart](https://www.quantstart.com/articles/Successful-Backtesting-of-Algorithmic-Trading-Strategies-Part-I/)
- [Can Algorithmic Traders Still Succeed at the Retail Level? — QuantStart](https://www.quantstart.com/articles/Can-Algorithmic-Traders-Still-Succeed-at-the-Retail-Level/)
- [Robustness Tests and Checks — BuildAlpha](https://www.buildalpha.com/robustness-testing-guide/)
- [Avoiding Overfitting — BacktestMe](https://backtestme.com/guides/avoiding-overfitting)
- [How Do Investment Strategies Perform After Publication? — QuantPedia](https://quantpedia.com/how-do-investment-strategies-perform-after-publication/)
- [Alpha Decay: What It Is and 3 Reasons It Occurs](https://medium.com/@dwongresearch0/alpha-decay-what-it-is-and-3-reasons-it-occurs-6cf942d916b4)
- [Walk-Forward Optimization Introduction — QuantInsti](https://blog.quantinsti.com/walk-forward-optimization-introduction/)
- [Systematic Trading — Robert Carver (Goodreads)](https://www.goodreads.com/book/show/25900953-systematic-trading)
- [Quantitative Trading — Ernest Chan (Wiley)](https://www.wiley.com/en-us/Quantitative+Trading:+How+to+Build+Your+Own+Algorithmic+Trading+Business,+2nd+Edition-p-9781119800064)
- [Personality Test for Successful Traders — QuantifiedStrategies](https://www.quantifiedstrategies.com/personality-test-for-traders/)
- [Uncorrelated Assets and Strategies — QuantifiedStrategies](https://www.quantifiedstrategies.com/uncorrelated-assets-and-strategies/)
- [Seven Sins of Quantitative Investing — Deutsche Bank / Hudson Thames](https://hudsonthames.org/wp-content/uploads/2022/01/DB-201409-Seven_Sins_of_Quantitative_Investing.pdf)
- [Survivorship Bias in Backtesting — LuxAlgo](https://www.luxalgo.com/blog/survivorship-bias-in-backtesting-explained/)
- [The Impact of Transaction Costs and Slippage — ResearchGate](https://www.researchgate.net/publication/384458498_The_impact_of_transactions_costs_and_slippage_on_algorithmic_trading_performance)
- [Sharpe Ratio for Algorithmic Trading — QuantStart](https://www.quantstart.com/articles/Sharpe-Ratio-for-Algorithmic-Trading-Performance-Measurement/)
- [Kelly Criterion — Frontiers in Applied Mathematics](https://www.frontiersin.org/journals/applied-mathematics-and-statistics/articles/10.3389/fams.2020.577050/full)
