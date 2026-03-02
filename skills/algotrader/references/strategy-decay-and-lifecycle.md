# Strategy Decay, Alpha Lifecycle, and Regime Change

## Why All Edges Die

No trading edge is permanent. This is not pessimism — it's the efficient market hypothesis working over longer timescales. Once a profitable pattern is discovered and exploited at scale, the exploitation itself reduces the inefficiency until it disappears or shrinks to the cost of discovery.

The question is not "will this edge decay?" but "how fast?" and "what's my plan when it does?"

---

## The Mechanics of Alpha Decay

### Crowding
When a profitable strategy becomes known, more capital flows into it. As more traders buy the same signals:
- The entry prices get worse (later buyers are competing with earlier ones)
- The exit prices get worse (everyone tries to exit the same crowded trade)
- The holding period shortens (trades resolve faster as more capital pushes prices to fair value)

**First mover advantage is real**: the first traders to discover an inefficiency capture most of the alpha. Followers get diminishing returns. Latecomers may get negative returns (entering a crowded trade at peak distortion).

### Regime Change
Markets change their character over time. A strategy optimized for one market regime may be wrong-footed in another:

| Old Regime | New Regime | Impact |
|-----------|-----------|--------|
| Low interest rates | High interest rates | Growth → value rotation; carry trades reverse |
| Low volatility | High volatility | Volatility-selling strategies blow up |
| Trending market | Sideways market | Trend following fails; mean reversion works |
| High liquidity | Liquidity crisis | Spreads widen; small-cap positions can't exit |

Structural regime changes (not cyclical ones) can permanently impair certain strategy types. A strategy built on assumptions from 2010-2020 (low rates, low vol, QE) may not work in a 2022+ environment.

### Arbitrage Completion
Some edges exist because of market structure inefficiencies that get fixed. Examples:
- Index inclusion effect: stocks added to S&P 500 used to gap up predictably; now front-running is so aggressive the effect has shrunk significantly
- Post-earnings announcement drift: reduced but not eliminated as more systematic traders exploit it
- Simple technical signals: 50/200 day moving average crossovers are so widely followed that their predictive value has degraded

---

## Typical Strategy Lifespans

Based on institutional and retail practitioner data:

| Strategy Type | Typical Lifespan | Why |
|--------------|-----------------|-----|
| Pure HFT (order book arbitrage) | Days to weeks | Extremely crowded; infrastructure advantages erode fast |
| Short-term intraday momentum | 3-6 months | Many competing algorithms; regime-sensitive |
| Swing / daily momentum | 6-18 months | Less crowded; requires monitoring |
| Trend following (weekly) | 1-3 years | Long holding periods smooth regime sensitivity |
| Macro / factor-based | 1-5+ years | Fundamental anchors slow the decay |
| Structural retail advantage (illiquid) | Multi-year | By definition uncrowded |

These are rough ranges, not guarantees. A trend following strategy can fail in a month during a severe regime change; a simple intraday strategy might persist 2+ years if the market structure supporting it remains stable.

---

## Diagnosing Decay vs. Normal Drawdown

This is the hardest practical skill. Abandoning a strategy during a normal drawdown (then watching it recover) is as damaging as holding a genuinely broken one.

### Metrics to Monitor in Live Trading

| Metric | Calculation | Warning Threshold |
|--------|-------------|------------------|
| Rolling Sharpe | 60-day window, annualized | Below 0.3 for 60+ days |
| Hit rate (win rate) | Rolling 50-trade window | Drops >15 percentage points from backtest |
| Average win/loss ratio | Rolling 50-trade window | Deteriorates >30% from backtest |
| Max adverse excursion | Average worst point of open trades | Systematically worsening |
| Profit factor | Gross wins / gross losses | Below 1.0 for 60+ days |

### The Regime Test

If the strategy is underperforming, ask:
1. **Has the market regime changed?** (Trending → sideways? Low vol → high vol?)
2. **Are all strategies underperforming, or just this one?** If all: regime change. If just one: strategy-specific issue.
3. **Are the individual trade characteristics similar to the backtest?** (Similar hold times, similar entry conditions?) If yes: normal drawdown. If dramatically different: structural change.

### When to Pause vs. Retire

**Pause**: Sharpe dropped but trade characteristics are similar to backtest and a regime change explains the underperformance. Wait for regime normalization. Resume with smaller size.

**Retire**: Trade characteristics have fundamentally changed (average holding period has halved, win rate has collapsed not cyclically but structurally), or the strategy has exceeded 2.5× its historical maximum drawdown.

---

## The Strategy Pipeline Model (Institutional Standard)

Professional quant desks (Two Sigma, Citadel, AHL) do not run one strategy. They run **hundreds of small, uncorrelated strategies simultaneously**, treating them as a portfolio of finite-lifespan assets.

### The Model

```
Research pipeline
├── 10+ ideas in development at any time
├── 3-5 strategies in paper trading / validation
├── 5-15 strategies live (small capital)
└── 2-5 strategies at full deployment
    └── Each monitored for decay; retired when Sharpe threshold breached
        └── Replaced from the validation queue
```

The key insight: **strategy retirement is expected and planned for, not a failure**. When a strategy decays, the response is "good, we expected this, here's the next one" — not panic or overhaul.

### For Retail Traders

Scale this model appropriately:
- Always have 1-2 strategies in development/validation while running live ones
- Never have all capital in a single strategy
- Treat each strategy as having a finite life; set a review trigger before deployment

---

## Refreshing and Improving Strategies

Decay is not always binary (works / doesn't work). Often strategies degrade gradually and can be improved:

### Regime Filtering
Add a regime filter that suspends trading when market conditions are unfavorable for the strategy type:
- Trend following: add a filter that checks if the longer-term trend is consistent
- Mean reversion: add a filter that avoids trading during strong trending periods
- Momentum: reduce position size when VIX is elevated above threshold

### Parameter Re-Optimization on Rolling Window
Periodically re-optimize parameters using only recent data (walk-forward in production):
- Every 6-12 months, run optimization on trailing 12-24 months
- If optimal parameters have shifted significantly, update
- If no parameter set yields positive expectancy, retire

### Universe Expansion
A strategy that worked on one instrument may work on similar instruments that haven't yet been crowded:
- If your US large-cap momentum strategy is decaying, test on European equities, EM equities, or smaller caps

---

## Post-Publication Academic Anomalies: The Quantpedia Evidence

QuantPedia research on 232 published trading strategies:
- Returns **decrease by 26-58%** in the 5 years after publication vs. the pre-publication period
- Decay is **gradual, not immediate** — significant alpha often persists for 3-5+ years post-publication
- Strategies with stronger theoretical backing decay more slowly
- **Implication**: academic papers are still useful idea sources; the returns just need haircuts

The decay pattern:
```
Year 0-1 post-publication:  ~85% of original return
Year 1-3 post-publication:  ~65% of original return
Year 3-5 post-publication:  ~50% of original return
Year 5+ post-publication:   ~30-40% of original return (often become "smart beta")
```

---

## Sources

- [Alpha Decay: What It Is and 3 Reasons It Occurs — Medium](https://medium.com/@dwongresearch0/alpha-decay-what-it-is-and-3-reasons-it-occurs-6cf942d916b4)
- [Alpha Decay for Systematic Traders — Maven Securities](https://www.mavensecurities.com/alpha-decay-what-does-it-look-like-and-what-does-it-mean-for-systematic-traders/)
- [How Do Investment Strategies Perform After Publication? — QuantPedia](https://quantpedia.com/how-do-investment-strategies-perform-after-publication/)
- [Signal Decay Analysis — MicroAlphas](https://microalphas.com/signal-decay-patterns/)
- [Why the Best Strategies Don't Last — TradingView](https://www.tradingview.com/chart/XAUUSD/hXkiMD44-Why-the-Best-Strategies-Don-t-Last-A-Quant-Truth/)
- [The Truth About Alpha Decay — LinkedIn / AlgoPro](https://www.linkedin.com/pulse/alpha-decay-algorithmic-traders-race-against-time-algopro-ai-nm0vf)
