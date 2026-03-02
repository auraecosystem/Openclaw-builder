# Portfolio of Uncorrelated Strategies: The Holy Grail

## Ray Dalio's Insight

Ray Dalio describes the portfolio of uncorrelated return streams as the "Holy Grail of investing":

> "If I have a diversified set of bets that are not correlated with each other, I can dramatically reduce my risk without sacrificing my returns."

The mathematics are extraordinary: **adding 7 uncorrelated strategies with even modest individual Sharpe ratios can halve total portfolio risk** without sacrificing expected return. This is one of the few free lunches in finance — diversification across truly uncorrelated systems.

---

## The Mathematics

### Why It Works

For two strategies with the same expected return (μ) and same volatility (σ), combined equally:

```
Portfolio volatility = σ × √(1 + ρ) / √2

Where ρ = correlation between strategies

At ρ = 1.0 (perfect correlation): portfolio vol = σ        (no benefit)
At ρ = 0.0 (zero correlation):    portfolio vol = σ / √2   (30% reduction)
At ρ = -1.0 (perfect hedge):      portfolio vol = 0        (theoretical perfect hedge)
```

With N uncorrelated strategies:
```
Portfolio vol = σ / √N
Portfolio Sharpe = individual Sharpe × √N
```

**Example**: 5 uncorrelated strategies each with Sharpe 0.7:
```
Portfolio Sharpe = 0.7 × √5 = 0.7 × 2.24 = 1.57
```

Five mediocre strategies, combined properly, outperform one good one.

### The Correlation Caveat

All of this assumes correlation holds in normal markets. **In crises, correlations spike** — the famous "correlations go to 1 in a crash." This is well-documented:

- The 2008 crisis: previously uncorrelated equity strategies suddenly became correlated as managers deleveraged everything simultaneously
- COVID crash (March 2020): similar spike in cross-strategy correlation

Mitigation: include **crisis-proof strategies** in the mix (long volatility, trend-following, gold, short-term mean reversion) that tend to decouple or profit during crises.

---

## Dimensions of Diversification

### By Strategy Type

| Strategy Type | Works Best In | Tends to Fail In |
|--------------|--------------|-----------------|
| Trend following | Sustained directional trends | Choppy, sideways markets |
| Mean reversion | Range-bound, overextended moves | Strong trends |
| Breakout | Consolidation → expansion | False breakouts, low-vol regime |
| Carry (yield/rate diff) | Stable regimes | Risk-off events |
| Volatility selling | Low-vol, stable markets | Vol spikes, crises |
| Long volatility | Crisis periods | Extended calm markets |

**Key**: some strategy types are explicitly negatively correlated. A mean-reversion strategy and a trend-following strategy often profit at different times. Running both smooths the equity curve.

### By Timeframe

| Timeframe | Characteristics |
|-----------|----------------|
| Intraday (seconds-minutes) | Microstructure-based; high turnover |
| Day trading (minutes-hours) | Intraday momentum; high frequency |
| Swing (1-10 days) | Technical/momentum; medium frequency |
| Position (weeks-months) | Fundamental/macro anchored |
| Long-term (months-years) | Factor-based; low turnover |

Different timeframes have minimal correlation even in the same asset class. An intraday scalper and a swing trader in the same stock are effectively running independent businesses.

### By Asset Class

| Asset Class | Correlation to US Equities (typical) |
|------------|--------------------------------------|
| US large-cap equities | 1.0 (baseline) |
| US small-cap equities | 0.8-0.9 |
| International equities | 0.7-0.85 |
| Investment-grade bonds | -0.1 to 0.2 |
| Gold | 0.0-0.2 |
| Commodities | 0.1-0.4 |
| Bitcoin (recent) | 0.3-0.6 |
| Forex | 0.0-0.3 |
| Managed futures (CTA) | -0.1 to 0.1 |

### By Regime Sensitivity

Design the portfolio so some strategies thrive when others struggle:
- **Inflationary period**: commodity trend following, short bonds
- **Deflationary recession**: long bonds, short equities
- **Bull market**: equity momentum, growth factor
- **Bear market**: short-selling, long volatility, trend following
- **Sideways**: mean reversion, options premium selling

---

## Practical Implementation

### Minimum Viable Diversification

For a retail algo trader, 3-5 strategies across at least 2 dimensions provides meaningful diversification:

**Example portfolio**:
1. Daily equity momentum (US large caps)
2. Mean reversion in S&P 500 futures (intraday)
3. Trend following in commodity futures (weekly)
4. Breakout in forex major pairs (daily)
5. Statistical arbitrage in sector ETF pairs

Each strategy is independently validated. The correlation matrix between them is computed and monitored.

### Measuring Correlation in Practice

```python
# Build correlation matrix from live trading returns (not backtest)
import pandas as pd
import numpy as np

# daily_returns: DataFrame with one column per strategy
corr_matrix = daily_returns.corr()

# Target: off-diagonal correlations < 0.3
# Warning: off-diagonal correlations > 0.5 (too similar)
```

Recalculate correlation every month. If two strategies that were uncorrelated are now correlated at 0.6+, reduce position in one or investigate why they've converged.

### Capital Allocation Across Strategies

The naive approach: equal capital to each strategy.

The better approach: equal **risk** (equal volatility contribution) to each strategy.

```python
# Risk parity allocation
strategy_vols = [s.rolling_annual_vol() for s in strategies]
total_inv_vol = sum(1/v for v in strategy_vols)
allocations = [(1/v) / total_inv_vol for v in strategy_vols]
```

Low-volatility strategies get more capital; high-volatility strategies get less. Net result: all strategies contribute equally to the portfolio's total risk.

---

## Correlation Spike Management

### Crisis Protocol

Pre-define what you do when correlations spike (which they will):

1. **Monitor rolling 30-day correlation** across all strategy pairs
2. **Trigger**: if >50% of strategy pairs show correlation >0.6, activate crisis protocol
3. **Crisis protocol**: reduce all position sizes by 30-50%; eliminate strategies that are now duplicating each other's risk
4. **Recovery**: revert to normal sizing when 30-day correlations return to <0.4

### Explicit Hedges

Consider dedicating 10-15% of risk budget to strategies that are explicitly negatively correlated with the core:
- Long VIX calls (cheap insurance; pays when everything else crashes)
- Short-term trend following (tends to capture crisis moves)
- Gold allocation (traditional crisis hedge)

These "portfolio insurance" strategies will drag performance in normal times but provide valuable diversification when correlation spikes kill the rest.

---

## The Carver Framework (pysystemtrade)

Robert Carver's open-source `pysystemtrade` implements this multi-strategy approach systematically:
- Each strategy is expressed as a "trading rule" with a forecast (scaled -20 to +20)
- Forecasts from multiple rules are combined with correlation-aware weights
- Position sizing adjusts dynamically based on volatility
- The full portfolio is diversification-multiplier adjusted to account for correlation

This is the most principled public framework for the multi-strategy approach.

---

## Sources

- [Uncorrelated Assets and Strategies — QuantifiedStrategies](https://www.quantifiedstrategies.com/uncorrelated-assets-and-strategies/)
- [Power of Diversified Trading — KJ Trading Systems](https://kjtradingsystems.com/algorithmic-trading-diversiifcation.html)
- [Ray Dalio's Holy Grail — StatOasis](https://statoasis.com/post/the-holy-grail-by-ray-dalio)
- [How to Build an Uncorrelated 2-Strategy Portfolio — Rogue Quant](https://roguequant.substack.com/p/what-happens-when-you-add-a-second)
- [Systematic Trading — Robert Carver](https://www.amazon.com/Systematic-Trading-designing-trading-investing/dp/0857194453)
- [The Science of Algorithmic Trading and Portfolio Management — ScienceDirect](https://www.sciencedirect.com/book/monograph/9780124016897/the-science-of-algorithmic-trading-and-portfolio-management)
