# Transaction Costs and Slippage: The Silent Strategy Killer

## The Most Underestimated Factor

> "One of the most prevalent beginner mistakes when implementing trading models is to neglect (or grossly underestimate) the effects of transaction costs on a strategy."
> — QuantStart

This is not a minor adjustment. **Many strategies that appear profitable in backtests are net losers once realistic costs are modeled.** A strategy with 0.15% expected profit per trade and 0.20% round-trip cost is not a 0.05% loser — it's a guaranteed ruin strategy run at scale.

---

## The Components of Transaction Costs

### 1. Commission
Fixed fee per trade charged by the broker.

- Modern retail brokers: ~$0 for US equities (Schwab, IBKR Lite)
- Futures at IBKR: ~$0.85-$2.05/contract depending on exchange
- Options: ~$0.65/contract
- Professional/institutional rates: negotiated, much lower at volume

**Impact**: smallest component for daily/swing strategies, but adds up at high frequency.

### 2. Bid-Ask Spread
The difference between the best price to buy (ask) and the best price to sell (bid). When you enter a market order, you immediately give up half the spread.

| Instrument | Typical Spread |
|-----------|---------------|
| S&P 500 futures (ES) | ~0.25 tick = 0.001% |
| Large-cap US equities | 0.01-0.05% |
| Small-cap US equities | 0.1-0.5% |
| Liquid forex (EUR/USD) | 0.01-0.03% |
| Exotic forex | 0.3-1.0%+ |
| Liquid crypto (BTC/ETH) | 0.01-0.05% |
| Mid-cap crypto altcoins | 0.2-2.0%+ |

**Round-trip cost of crossing the spread**: full spread on entry + full spread on exit.

### 3. Slippage
The difference between the price at the moment your signal fires and the price at which your order actually fills.

**Sources of slippage**:
- **Latency**: by the time your order reaches the exchange, the price has moved
- **Market microstructure**: your market order consumes the best available liquidity; if the order is larger than the top-of-book quantity, you fill at progressively worse prices
- **Momentum effect**: if you're buying into a rising market (momentum strategy), the price continues moving against you during execution

**Slippage is directional by strategy type**:
- Momentum strategies: systematically worse slippage (you're chasing)
- Mean reversion strategies: systematically better slippage (you're providing liquidity)

### 4. Market Impact
When your position is large relative to typical volume, your own order moves the market. This is non-linear — doubling position size more than doubles market impact.

**Rule of thumb**: avoid trading more than 1% of the instrument's average daily volume in a single day.

For a stock with 1M shares/day average volume, max comfortable daily position: 10,000 shares. Above this, your own order starts to meaningfully move the price.

**Mathematical model**: Market impact ≈ σ × √(Q/V) where σ = daily volatility, Q = order quantity, V = average daily volume. This is the square-root market impact model used by most institutional desks.

### 5. Overnight Financing (Margin Interest)
For leveraged positions held overnight:
- US equities: typically 5-8% annual rate on borrowed capital
- Futures: embedded in the futures roll, negligible for outright long/short
- Crypto: typically 0.01-0.05% per day = 3.65-18.25% annualized

For a leveraged strategy with 2:1 leverage, financing costs are ~3-4% annual drag.

### 6. Short Borrow Costs
For short equity positions:
- Easy-to-borrow stocks: 0.3-1% annual
- Hard-to-borrow (high short interest, low float): 5-50%+ annual
- Some heavily shorted stocks: >100% annual borrow cost

Any short strategy must account for borrow costs explicitly. A short strategy on a stock with 20% annual borrow cost needs to generate 20%+ annual alpha just to break even on the short leg.

---

## Strategy-Specific Cost Impacts

### Momentum / Trend Following
Highest cost impact of all strategy types.
- Entries occur when price is already moving — you're late by definition
- Market orders fill at the worst point of the recent move
- Example: stock gaps up 2% at open; by the time your signal fires and order reaches the exchange, it's 2.3% above previous close. You bought 0.3% worse than the signal.
- **Slippage can consume 30-60% of gross edge** for daily momentum strategies with short holding periods

### Mean Reversion
Better cost profile.
- You're buying weakness, selling strength — providing liquidity to panicking sellers
- Limit orders (resting at the mean, not chasing) can earn the spread rather than paying it
- Market impact is smaller because you're acting against the current flow

### Statistical Arbitrage / Pairs Trading
Symmetric costs (long + short simultaneously).
- Two legs means 2x spread + 2x slippage
- Must earn enough on the convergence to pay both legs
- Works best on highly correlated pairs with liquid, tight-spread instruments

### High-Frequency Trading (HFT)
Cost is everything.
- A 1ms latency advantage can mean the difference between paying spread and earning spread
- Transaction cost models need to be in basis points (0.01%), not percent
- Retail traders cannot compete here: co-location, FPGA hardware, and direct market access required

---

## Modeling Costs in Your Backtest

### Minimum Realistic Cost Model (Daily Strategies)

```python
round_trip_cost = spread + slippage + commission

# Conservative estimate per trade:
spread = 0.05%          # for liquid mid-caps
slippage = 0.05-0.10%   # momentum adds; mean-rev subtracts
commission = 0.0-0.02%  # effectively zero at major retail brokers

round_trip_cost ≈ 0.10-0.20%
```

If your strategy makes 200 trades/year with average 0.15% round-trip cost:
- Total annual cost drag: 30%

A strategy with 45% gross annual return becomes 15% net. This changes everything about risk-adjusted analysis.

### Applying Costs in Event-Driven Backtests

```python
# At each trade entry:
fill_price = signal_price * (1 + slippage + half_spread)  # for long entry
fill_price = signal_price * (1 - slippage - half_spread)  # for short entry

# At each trade exit:
fill_price = signal_price * (1 - slippage - half_spread)  # for long exit
fill_price = signal_price * (1 + slippage + half_spread)  # for short exit

# Subtract commission per trade:
trade_pnl -= commission_per_trade
```

### Advanced: Quadratic Market Impact Model

For larger positions, linear cost models underestimate impact:

```
impact = alpha × σ × (Q/V)^0.5

Where:
  alpha = market impact coefficient (~0.1-0.5, calibrated empirically)
  σ = daily volatility of the instrument
  Q = order size (shares)
  V = average daily volume (shares)
```

Implementing this matters when your trade size exceeds ~0.1% of average daily volume.

---

## Paper Trading as Cost Calibration

Before going live, paper trade for **30+ days in live market conditions** (not replay). The purpose is not to validate the strategy (that's the backtest's job) but to measure:

- Actual fill prices vs. theoretical fill prices
- Latency from signal to fill
- Partial fill frequency
- Gap openings that bypass stop orders

After 30 days of paper trading, compare the distribution of actual fills vs. your backtest's assumed fills. The gap is your actual slippage model. Update your backtest with this calibrated number and recheck whether the strategy remains viable.

---

## The Break-Even Trade Frequency

Given a fixed per-trade cost, there is a minimum trade frequency above which costs dominate:

```
max_viable_trades_per_year = annual_gross_edge / round_trip_cost

Example:
annual_gross_edge = 15%
round_trip_cost = 0.15%
max_trades = 100/year (roughly 2/week)
```

Above this trade frequency with the same gross edge, the strategy loses money. This is why pure intraday strategies require either massive edge per trade or near-zero execution costs (HFT infrastructure).

---

## Sources

- [Impact of Transaction Costs and Slippage — ResearchGate](https://www.researchgate.net/publication/384458498_The_impact_of_transactions_costs_and_slippage_on_algorithmic_trading_performance)
- [Successful Backtesting Part II — QuantStart](https://www.quantstart.com/articles/Successful-Backtesting-of-Algorithmic-Trading-Strategies-Part-II/)
- [Trading Slippage: Minimize Hidden Costs — LuxAlgo](https://www.luxalgo.com/blog/trading-slippage-minimize-hidden-costs/)
- [Slippage: Hidden Costs in Backtesting — FasterCapital](https://www.fastercapital.com/content/Slippage--The-Hidden-Costs--Accounting-for-Slippage-in-Backtesting.html)
- [Transaction Cost Analysis in Crypto — Anboto Labs / Medium](https://medium.com/@anboto_labs/slippage-benchmarks-and-beyond-transaction-cost-analysis-tca-in-crypto-trading-2f0b0186980e)
- [The Paradox of the Pre-Trade Cost Model — Quantitative Brokers](https://www.quantitativebrokers.com/blog/the-paradox-of-the-pre-trade-cost-model)
- [Backtesting Tutorial: Transaction Costs — Hudson & Thames](https://github.com/hudson-and-thames/backtest_tutorial/blob/main/Intro_Transaction_Costs.ipynb)
