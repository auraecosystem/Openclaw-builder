# Backtesting: The Seven Deadly Sins and How to Avoid Them

## Why This Matters

The German quant blog *The Financial Hacker* estimates that **90% of backtests fail in live trading**. The gap between backtest performance and live performance is not bad luck — it is almost always traceable to one or more of the following systematic errors. Understanding these is more valuable than any strategy idea.

A backtest is a hypothesis test, not proof. Its job is to **reject bad ideas quickly**, not to confirm that you've found gold.

---

## Sin 1: Survivorship Bias

### What It Is
You test your strategy only on stocks (or assets) that currently exist in your data source. Companies that went bankrupt, got delisted, or were acquired are excluded. Since these tend to be the worst performers, your universe is artificially clean.

### The Impact
Overstates annual equity returns by **1-4% per year** on average. Over a 10-year backtest, this is a 10-48% performance inflation that vanishes completely in live trading.

### Examples
- Testing a "buy cheap stocks" strategy on the current S&P 500 membership, ignoring that cheap stocks have a higher failure rate
- Testing a crypto strategy only on coins still trading today, excluding the hundreds that went to zero

### The Fix
- Use **point-in-time databases**: Compustat (academic), Norgate Data (retail), CRSP (institutional)
- For crypto: include historical data for delisted tokens
- For forex: include pairs that were discontinued
- Explicitly verify: "Is every asset in my test universe actually tradeable at that historical date?"

---

## Sin 2: Look-Ahead Bias

### What It Is
Your strategy uses information that would not have been available at the moment of the trading decision. The backtest "sees the future" — and of course it looks profitable.

### Examples
- Using today's closing price to generate a signal, then executing at today's open (the close wasn't known at open)
- Using quarterly earnings data without accounting for the reporting lag (earnings reported in February use data from December)
- Calculating a 20-day moving average at bar close and entering the trade at bar close rather than the next open
- Normalizing data using statistics computed over the full dataset (future data included in the normalization)

### The Impact
Can create spectacular-looking backtests with Sharpe ratios >5 that immediately collapse in live trading.

### The Fix
- **Shift all signals by at least one bar**: if signal fires at close, execute at next open
- **Use point-in-time financial data**: account for the gap between fiscal period end and public reporting date (typically 30-75 days for earnings)
- **Be paranoid about data alignment**: when joining two datasets, verify timestamps are aligned to the same timezone and there's no forward contamination
- **Audit your normalization**: any feature normalized using full-history statistics is look-ahead contaminated. Use expanding window normalization instead

---

## Sin 3: Overfitting and Curve Fitting

### What It Is
You optimize strategy parameters so thoroughly that the strategy perfectly describes historical noise rather than persistent signal. The equity curve looks beautiful because every parameter was tuned to fit the wiggles in that specific dataset.

### The Warning Signs
- More than 5-7 parameters in your strategy
- Backtest Sharpe ratio > 3.5 on first pass (genuine edges rarely exceed 2.5 before costs)
- Parameters that are suspiciously precise (e.g., "exactly 14 bars" and "exactly 2.37x ATR")
- Strategy fails immediately on the first month of live trading
- Performance is dramatically different on different but equivalent assets

### The Impact
The classic Knight Capital case: a trading algorithm optimized on historical data deployed live and lost **$440 million in 45 minutes** in 2012 before humans could shut it down.

### The Fix
- **Fewer parameters**: a 2-parameter strategy that works is worth 100x a 15-parameter strategy that looks great
- **Conceptual validity**: if you can't explain *why* the strategy works in plain English, it's probably overfitted
- **Perturb parameters ±20%**: if changing the stop loss from 2.0 ATR to 2.4 ATR causes performance to collapse, the strategy is fragile
- **Walk-forward analysis**: test on rolling unseen windows (see separate document)
- **Monte Carlo permutation**: reshuffle trade order 1,000 times — does the strategy still show positive expectancy across permutations?

---

## Sin 4: Data Snooping (p-Hacking / Multiple Comparisons)

### What It Is
You run 200 strategy variations, find the 3 that look good, and report only those. This is identical to flipping a coin 200 times and finding a streak of 5 heads — it's expected by random chance.

### The Math
If you test 100 strategies with a significance threshold of p < 0.05, you should expect **5 to look significant purely by chance**. If you then present those 5 as "validated" without disclosing the 95 failures, your research is scientifically fraudulent (even if unintentionally).

### How Traders Do This Accidentally
- Trying many indicator combinations until something backtests well
- Testing on the same validation set multiple times, adjusting after each failure
- Keeping a "best" version and discarding alternatives without reporting them

### The Fix
- **Pre-register your hypothesis** before testing: write down exactly what you're testing and why before touching the data
- **Bonferroni correction**: if you test N strategies, require p < 0.05/N for any single one to be considered significant
- **Use a truly unseen holdout set**: a dataset you've never looked at, ever, for final validation
- **Disclose all strategies tested, not just the winners**

---

## Sin 5: Transaction Cost Underestimation

### What It Is
Backtests assume you buy at the close price or mid-price with zero friction. In reality, every trade costs money: commissions, the bid-ask spread, slippage on entry, and market impact.

### The Impact by Strategy Type
- A strategy with 0.15% expected profit per trade with 0.20% realistic round-trip cost is a net loser
- High-frequency strategies (50+ trades/day) can have 70-90% of gross profit consumed by costs
- **Momentum strategies** are hardest hit: you're chasing a moving price, so your fill is systematically worse than the signal price
- **Mean reversion strategies** are somewhat better: you're buying into weakness so fills trend in your favor

### Components Often Omitted

| Cost | Typical Range | Notes |
|------|--------------|-------|
| Commission | $0-$5/trade | Near-zero at major brokers now |
| Bid-ask spread | 0.01-0.5% | Higher for illiquid assets |
| Slippage | 0.05-0.3% | Momentum strategies: worse; mean reversion: better |
| Market impact | Nonlinear with size | Quadratic model most accurate |
| Overnight financing | 0.01-0.05%/day | For leveraged positions |
| Short borrow cost | 0.5-10%+/year | For short positions |

### The Fix
- For daily strategies: model minimum 0.1-0.2% round-trip cost unless you have specific data
- For intraday: model spread explicitly; include realistic fill assumptions
- **Paper trade before going live** — the gap between assumed and actual fills will surprise you
- For large positions: model market impact as nonlinear (quadratic)

---

## Sin 6: Period Selection Bias

### What It Is
You test only on periods that happen to be favorable to your strategy's type. A trend-following system tested only on 2010-2021 (a sustained bull market) looks very different than one tested on 2000-2021 (which includes two major crashes).

### Examples
- Testing a long-only strategy from 2010-2021 and concluding it's robust
- Testing a volatility-selling strategy on 2012-2019 (historically low vol) without including 2008 or 2020
- Testing during low-interest-rate regimes without considering how performance changes when rates rise

### The Fix
- Include at least one full market cycle (bull + bear + sideways)
- **Mandatory stress periods to include**: 2000-2002 (dot-com crash), 2008-2009 (financial crisis), 2020 (COVID), 2022 (rate hike cycle)
- Ask: "How would this strategy have behaved during the worst 10% of months in history?"
- If you don't have enough data to cover multiple regimes, acknowledge this as a major risk

---

## Sin 7: Incomplete or Low-Quality Data

### What It Is
Your data source has errors, gaps, or limitations that cause the backtest to be testing something that never actually happened.

### Common Data Quality Problems
- **Corporate actions not adjusted**: stock split adjustments, dividend adjustments not applied, causing phantom price jumps
- **OHLC assumptions**: daily OHLC data assumes you can fill at the open, close, high, or low — but intraday execution may have been impossible at those exact prices
- **Timezone misalignment**: a signal computed in UTC applied to prices in US Eastern Time creates systematic look-ahead in certain hours
- **Stale data**: prices that haven't updated (especially for illiquid instruments) creating false signals
- **Vendor errors**: data vendors have systematic errors in specific periods or instruments — always cross-reference

### The Fix
- Cross-validate against a second data source for any asset you plan to trade
- Verify all corporate action adjustments
- Use adjusted close prices (not raw) for longer-term equity strategies
- For intraday: use tick-level data or minute bars with realistic fill models, not daily OHLC

---

## The Correct Data Split Protocol

```
Total historical data
├── Training set (60-70%)
│   └── Strategy development, initial parameter search
├── Validation set (15-20%)
│   └── Parameter tuning — but only ONE pass, not iterative
└── Holdout set (15-20%) — NEVER TOUCH until final verdict
    └── Touch once, final evaluation, decision to deploy or discard
```

**If the holdout set result is disappointing, you discard the strategy entirely.** You do not go back and re-optimize. The holdout set is sacred — using it as feedback and reoptimizing converts it into another training set, invalidating its purpose.

For recent data (last 12-24 months), always preserve as holdout. Markets change; testing on recent periods is more relevant than ancient periods.

---

## Sources

- [The Seven Sins of Quantitative Investing — Deutsche Bank / Hudson Thames](https://hudsonthames.org/wp-content/uploads/2022/01/DB-201409-Seven_Sins_of_Quantitative_Investing.pdf)
- [Survivorship Bias in Backtesting — LuxAlgo](https://www.luxalgo.com/blog/survivorship-bias-in-backtesting-explained/)
- [Common Pitfalls in Backtesting — Medium / Pham The Anh](https://medium.com/funny-ai-quant/ai-algorithmic-trading-common-pitfalls-in-backtesting-a-comprehensive-guide-for-algorithmic-ce97e1b1f7f7)
- [Why 90% of Backtests Fail — The Financial Hacker](https://financial-hacker.com/why-90-of-backtests-fail/)
- [Successful Backtesting Part I — QuantStart](https://www.quantstart.com/articles/Successful-Backtesting-of-Algorithmic-Trading-Strategies-Part-I/)
- [Successful Backtesting Part II — QuantStart](https://www.quantstart.com/articles/Successful-Backtesting-of-Algorithmic-Trading-Strategies-Part-II/)
- [Backtesting Biases — Auquan / Medium](https://medium.com/auquan/backtesting-biases-and-how-to-avoid-them-776180378335)
- [Curve Fitting in Trading — QuantifiedStrategies](https://www.quantifiedstrategies.com/curve-fitting-trading/)
