# Short-Horizon Crypto Momentum: A Quant's Blueprint

How a quantitative trader would implement Qullamaggie-style breakout/momentum on crypto with short holding periods, optimizing for CAGR on a small portfolio. Reactive, not predictive — riding waves as they happen.

---

## The Core Idea, Stated Precisely

**What Qullamaggie actually does** (distilled to its quantitative essence):

1. **Rank** the universe by recent relative strength (top 1-2% over 1/3/6 months).
2. **Filter** for stocks in a volatility contraction (consolidation range <15%, declining volume).
3. **Enter** when price breaks the consolidation high on above-average volume.
4. **Stop** at the low of the breakout day (risk = 1 ATR or less).
5. **Exit quick partial** (20-50% of position) after 3-5 days for the "predictable first move."
6. **Trail remainder** on the 10 or 20-day MA. Exit on close below.

The genius is in the asymmetry: tight stops (risk ~1 ADR) vs. unlimited upside on runners. Win rate is 25-35%. Profitability comes from the ~10-20% of trades that produce outsized gains.

**Adapting this for crypto with short horizons and max CAGR:**

The same pattern recognition applies — volatility contraction → expansion — but crypto's 24/7 market, higher volatility, and shorter momentum cycles let you compress the timeline. Instead of holding runners for weeks/months, you're capturing 2-10 day moves and rapidly recycling capital into the next setup.

---

## Why Short-Horizon Momentum Works in Crypto

### The Academic Case

- **2-week momentum** is the optimal crypto horizon (vs. 12-month for equities). The CTREND factor (JFQA, 2025) combining price and volume across multiple horizons reliably predicts crypto returns across 3,000+ coins.
- **Volatility is the raw material**: BTC annualized vol ~60-80%, altcoins 100-200%+. A 5-day hold in crypto can capture the same percentage move as a 3-week hold in equities.
- **Inefficiencies persist**: crypto markets remain less efficient than equities. Retail-dominated altcoins particularly so. On-chain metrics confirm institutional flow leads price by hours to days.
- **Momentum crashes are real** but regime-filterable. Time-series momentum + regime detection avoids the worst drawdowns.

### Why "Reactive, Not Predictive" Is Correct

Trend following is inherently reactive — it doesn't predict, it responds. This is a feature, not a bug:

- You don't need to know *why* a coin is moving. You need to detect that it *is* moving, in an orderly way, from a consolidated base, on volume.
- False breakouts are filtered by requiring volume confirmation, volatility contraction prior, and relative strength context.
- The stop-loss handles the rest: when you're wrong, you lose 1 ATR. When you're right, you ride.

---

## The Quantitative Framework

### Step 1: Universe Selection

**Goal**: narrow the ~100-500 liquid tokens to the ~5-20 that are exhibiting relative strength and base-building behavior right now.

| Filter | Crypto Implementation | Why |
|--------|----------------------|-----|
| Liquidity floor | 24h volume > $5M (or $10M for concentrated portfolio) | Ensures executable entries/exits without excessive slippage |
| Relative strength ranking | Top 20% by return over 7d, 14d, 30d (composite score) | Identifies the leaders; shorter lookbacks than equities because crypto momentum cycles are faster |
| Trend confirmation | Price > 20-day EMA, 20-day EMA > 50-day EMA | Equivalent to Minervini's Stage 2 / Qullamaggie's QQQ filter; ensures you're buying uptrends |
| Proximity to highs | Price within 25% of 90-day high | Buying leaders near highs, not broken charts bouncing from lows |
| BTC regime filter | BTC > 50-day SMA (or BTC 10 EMA > 20 EMA) | Crypto equivalent of QQQ regime filter; go flat or reduce size in bearish regimes |

**Crypto-specific adjustments vs. equities:**
- Shorter lookback windows (7/14/30 days vs. 1/3/6/12 months) because crypto momentum decays faster.
- Volume in USD, not shares. Crypto volume data is unreliable on many exchanges — use Binance/Coinbase data only.
- Smaller universe (100-200 liquid tokens vs. 5,000+ stocks) means you're selecting from a pre-filtered pool. RS percentile thresholds should be wider (top 20% instead of top 2%).

### Step 2: Pattern Detection (Volatility Contraction → Expansion)

**What you're looking for**: a token that has already moved significantly, then paused in a tight range with declining volume, about to break out and continue.

**Quantitative definition of a "base":**

```
Consolidation Range = (max(high, N days) - min(low, N days)) / min(low, N days)
Volume Contraction  = avg_volume(last 5 days) / avg_volume(last 20 days)
ATR Contraction     = ATR(5) / ATR(20)
```

**Criteria for a valid base:**

| Metric | Threshold | Rationale |
|--------|-----------|-----------|
| Consolidation range | < 15-20% (crypto needs wider than equities' <15%) | Tight enough to indicate absorption, not so tight it's dead |
| Consolidation duration | 3-15 days (shorter than equities' 2-8 weeks) | Crypto bases compress faster; >15 days may indicate breakdown |
| Volume contraction ratio | < 0.7 (current 5d avg < 70% of 20d avg) | Declining volume = selling pressure exhausted |
| ATR contraction ratio | < 0.8 (current volatility contracting) | The "spring" is coiling |
| Prior move | > 20% in prior 14-30 days | Must have already demonstrated momentum (something to consolidate from) |
| Higher lows in base | Yes (quantify: base_low(last 3 days) > base_low(first 3 days)) | Supply absorbing at progressively higher prices |

**This is the VCP (Volatility Contraction Pattern) in quantitative terms.** These indicators (`vcp_num_contractions`, `vcp_last_contraction_pct`, `vcp_tightening_ratio`, `vcp_vol_trend`) are pre-computed and cached for fast parameter sweeping in NautilusTrader backtests.

### Step 3: Entry Signal

**Trigger**: price breaks above the consolidation high on above-average volume.

```
Entry Signal = (close > max(high, consolidation_period))
             AND (volume > 1.5 * avg_volume_20d)
             AND (all universe filters still pass)
```

**For faster entries (intraday resolution)**:
- Use hourly or 4-hour candles to detect breakouts sooner than daily close.
- Enter on the first hourly candle that closes above the consolidation high with volume confirmation.
- Crypto's 24/7 market means breakouts can happen any time. NautilusTrader supports 5-minute (and higher-frequency) bar data for sub-daily strategy testing.

**What NOT to enter**:
- Breakout already extended > 1 ATR from the base (chasing; Qullamaggie explicitly avoids this).
- Volume spike but price immediately reverses (failed breakout — price must sustain above the breakout level).
- During BTC bearish regime (regime filter off).

### Step 4: Position Sizing for Max CAGR

This is where the small portfolio and CAGR maximization goals shape the strategy differently from Qullamaggie's approach:

**Qullamaggie's sizing** (for reference):
- 0.25-1% risk per trade, 15-20 positions, 5-25% per position.
- Designed for a $50M+ portfolio where capital preservation dominates.

**Small portfolio adaptation for max CAGR:**

Use **fractional Kelly criterion** for sizing. The idea: bet proportional to your edge, larger when edge is stronger, smaller when weaker.

```
Full Kelly: f* = (p * b - q) / b

Where:
  p = win probability (~0.30-0.35 for breakout strategies)
  q = loss probability (1 - p = 0.65-0.70)
  b = win/loss ratio (avg_win / avg_loss; typically 3-5x for breakout strategies)

Example: p=0.32, b=4.0 → f* = (0.32 * 4.0 - 0.68) / 4.0 = 0.15 (15% of capital)
```

**Use half-Kelly or quarter-Kelly** in practice:
- **Half-Kelly** (7-8% risk per trade): aggressive but survivable. Reduces CAGR by ~25% vs full Kelly but cuts max drawdown roughly in half.
- **Quarter-Kelly** (3-4% risk per trade): conservative-aggressive. Good starting point for live trading.

**Practical position sizing table:**

| Account Risk Tolerance | Risk Per Trade | Max Positions | Typical Position Size | Expected Max DD |
|------------------------|---------------|---------------|----------------------|-----------------|
| Aggressive (max CAGR) | 2-3% | 3-5 | 15-30% | 40-60% |
| Moderate-aggressive | 1-2% | 5-8 | 10-20% | 25-40% |
| Moderate | 0.5-1% | 8-12 | 5-15% | 15-25% |

**For a small portfolio maximizing CAGR**: 3-5 concentrated positions at 15-30% each, with 2-3% risk per trade. Accept 40-60% peak drawdowns as the cost of high compound growth. This is emotionally brutal but mathematically optimal for growth (half-Kelly).

**Critical**: the position size is calculated from the stop distance, not arbitrary:

```
position_size = (account * risk_pct) / (entry_price - stop_price)
position_pct  = (position_size * entry_price) / account
```

If the stop is tight (close to entry), the position is large. If the stop is wide, the position is small. This automatically adjusts for volatility.

### Step 5: Stop Loss

**Initial stop**: low of the breakout candle (daily or hourly, depending on entry timeframe).

**Constraint**: stop must not be wider than 1.5x ATR(14). If the natural stop (breakout candle low) is wider than 1.5x ATR, either:
- Pass on the trade (stop too wide for the risk budget), or
- Enter on a pullback/retest of the breakout level for a tighter stop.

**Crypto adjustment**: use 1.5x ATR instead of equities' 1x ADR because crypto has more noise/wicks.

**Hard stop**: always a market order, never a limit. Same principle as Qullamaggie: when the thesis is broken, exit immediately.

### Step 6: Exit Strategy (Optimized for Short Horizons)

This is the key divergence from standard Qullamaggie: instead of trailing for weeks/months, you're harvesting faster moves and recycling capital.

**Three-tier exit system:**

| Tier | Action | Timing | Rationale |
|------|--------|--------|-----------|
| Quick profit | Sell 30-50% of position | After 1-3x ATR profit (typically 1-3 days) | Captures the "predictable first move" (Qullamaggie's exact phrase) |
| Move stop to breakeven | On remaining 50-70% | After quick profit taken | Now a free trade; eliminates downside |
| Trailing stop | ATR-based trail on remainder | Exit when close < (highest_close - 2 * ATR(14)) | Rides the trend while locking in gains |

**Why ATR-based trailing instead of MA-based:**
- MA-based trailing (10/20-day SMA) is designed for multi-week holds. For 2-10 day holds, it's too slow.
- ATR trailing adapts to current volatility. In crypto's variable-vol environment, this matters.
- Suggested: 2x ATR(14) trailing from highest close since entry.

**Time stop**: if position hasn't moved 1x ATR in profit within 5 days, close it. Capital is the scarce resource. Dead money kills CAGR.

**Maximum hold**: 10 trading days (daily timeframe) or ~15-20 bars (4-hour timeframe). If still profitable but not trailing-stopped, take profits and move on. The goal is capital velocity.

### Step 7: Capital Recycling (The CAGR Engine)

**This is the most important concept for maximizing CAGR on a small portfolio.**

CAGR = f(edge_per_trade × number_of_trades × compounding)

You maximize CAGR by:
1. **Maintaining edge per trade** (don't degrade entry quality for volume).
2. **Increasing trade frequency** (shorter holds → more trades → more compounding events).
3. **Compounding fully** (profits immediately become part of the next trade's capital base).

**Trade frequency target**: 2-4 trades per week (when setups are available). This is ~100-200 trades per year — well within the range needed for statistical significance.

**Capital velocity calculation:**

```
If avg hold = 5 days and you run 4 positions:
  → Each $ of capital turns over ~50x per year
  → With 0.5% edge per trade after costs:
     50 turns × 0.5% = 25% before compounding
  → With compounding: significantly higher

If avg hold = 15 days (standard swing):
  → ~17 turns per year
  → 17 × 0.5% = 8.5% before compounding
```

Cutting average hold from 15 to 5 days roughly triples capital velocity. This is why short horizons maximize CAGR — not because each trade is more profitable, but because you compound more frequently.

**The constraint**: you can only increase trade frequency if sufficient setups exist. In crypto with 100+ liquid tokens, this is usually not the binding constraint during active markets. During quiet/bearish periods, setups dry up and you sit in cash (regime filter handles this).

---

## Regime Management: When to Trade and When to Sit

### The BTC Regime Filter

Crypto is highly correlated to BTC. During BTC downtrends, altcoin breakouts fail at much higher rates.

**Implementation:**

```
Bull regime   = BTC close > SMA(50) AND SMA(20) > SMA(50)   → Full position sizing
Neutral       = BTC close > SMA(50) but SMA(20) < SMA(50)   → Half position sizing
Bear regime   = BTC close < SMA(50)                          → No new longs; cash
```

This is the crypto analog of Qullamaggie's "QQQ 10 EMA > 20 EMA" filter.

**Refinement — volatility regime overlay:**

```
Low vol   = BTC ATR(14) / close < 2%    → Breakouts more likely to sustain; full size
Mid vol   = BTC ATR(14) / close 2-4%    → Normal conditions
High vol  = BTC ATR(14) / close > 4%    → Breakouts whipsaw; reduce size or sit out
```

Volatility contraction in BTC itself signals the next large move. When BTC vol compresses, be ready with a watchlist — the breakout (in either direction) is coming.

### Altcoin Rotation Timing

Capital flows in crypto follow a predictable cascade:
1. BTC leads (dominance rising → money enters crypto via BTC).
2. ETH follows (BTC consolidates → money rotates to ETH).
3. Large-cap alts (ETH consolidates → rotation into SOL, AVAX, etc.).
4. Mid/small-cap alts (final phase → speculative froth).

**For the breakout system**: you see the most setups in phases 2-3. Phase 4 has the highest CAGR potential but also the highest crash risk. Phase 1 may only give you BTC breakout trades.

Track BTC dominance as a rotation signal:
- BTC.D rising = stay in BTC or cash.
- BTC.D falling from resistance = look for alt breakouts (the setups will appear in your scans).

---

## Transaction Cost Model

Short horizons mean more trades. Costs compound against you.

**Realistic cost model per round trip:**

| Component | Binance (VIP) | Binance (Standard) | Notes |
|-----------|--------------|-------------------|-------|
| Maker + Taker fees | 0.04-0.08% | 0.10-0.20% | Limit entry (maker) + market exit (taker) |
| Slippage (entry) | 0.02-0.05% | 0.05-0.10% | Larger for small-cap alts |
| Slippage (exit) | 0.03-0.08% | 0.08-0.15% | Stop exits have worse fills |
| **Total round trip** | **0.09-0.21%** | **0.23-0.45%** | Per trade |

**Annual cost at 150 trades/year:**
- VIP: 150 × 0.15% = 22.5% of capital in costs.
- Standard: 150 × 0.34% = 51% of capital in costs.

**This is why exchange fee tiers matter enormously for short-horizon strategies.** The difference between VIP and standard is ~28% of annual capital in costs. For max CAGR, you MUST minimize fees via:
- High volume tier qualification (or BNB fee discount on Binance).
- Limit orders for entries where possible.
- Trading only liquid pairs (top 30-50 by volume) to minimize slippage.

**Breakeven analysis**: if average winner is +5% and win rate is 32%, and average loser is -1.5%, then:
```
Expected value per trade = (0.32 × 5%) - (0.68 × 1.5%) = 1.6% - 1.02% = 0.58%
After costs (standard): 0.58% - 0.34% = 0.24% net
After costs (VIP): 0.58% - 0.15% = 0.43% net
```

VIP-tier costs nearly double the net edge. This matters.

---

## Risk Management for Small Concentrated Portfolios

### Portfolio-Level Controls

| Rule | Value | Rationale |
|------|-------|-----------|
| Max correlated exposure | 2 positions in same sector/narrative | Altcoins within the same narrative (e.g., 3 AI tokens) move together |
| Max portfolio heat | 10-15% of capital at risk simultaneously | With 3-5 positions at 2-3% risk each |
| Drawdown circuit breaker | At -15%: halve position sizes. At -25%: stop trading for 1 week | Prevents ruin during regime changes |
| Consecutive loss limit | 5 consecutive losers → pause for 24h, re-evaluate regime | Prevents tilt and over-trading during hostile conditions |
| Daily loss limit | -3% of equity → done for the day | Hard stop to prevent blowout days |

### Correlation Risk

In crypto, correlations spike during selloffs. Your "diversified" 5 altcoin positions can all gap down 15% simultaneously.

**Mitigation:**
- BTC regime filter (most important — catches the macro drawdowns).
- Max 2 positions in the same "narrative cluster" (AI tokens, L2s, meme coins, DeFi, etc.).
- Track 30-day rolling correlation of your positions to BTC. If all positions have >0.8 BTC correlation, you effectively have one big BTC-beta bet.
- Consider 1 position in a BTC/ETH breakout + 2-3 in diverse altcoin sectors rather than 5 correlated alts.

---

## Parameter Optimization for NautilusTrader

Mapping this framework to the NautilusTrader strategy's parameters:

| Parameter | Equity Default | Crypto Short-Horizon | Why |
|-----------|---------------|---------------------|-----|
| `rs_pct` | 0.02 (top 2%) | 0.15-0.25 (top 15-25%) | Only 100 tokens; top 2% = 2 tokens |
| `vol_ratio` | 1.5 | 1.5-2.0 | Same logic; confirm breakout with volume |
| `max_range_pct` | 0.15 | 0.15-0.25 | Crypto bases are wider; 15-25% range still qualifies |
| `max_dist_52w` | 0.25 | 0.25-0.35 | Crypto is more volatile; widen slightly |
| `min_adv` | $150M | $5M-$20M | Crypto liquidity is lower |
| `slippage_k` | 0.10 | 0.10-0.15 | Add explicit exchange fee (~0.075%) on top |
| `min_prior_move` | 0.30 | 0.20-0.40 | Prior move requirement; 20-40% in crypto |
| `max_sma_ext` | 0.10 | 0.15-0.25 | Allow more extension in volatile crypto |
| `regime` | true (SPY > 200d SMA) | true (BTC > 200d or 50d SMA) | Critical for crypto |
| `risk_pct` | 0.005 | 0.015-0.025 | Aggressive for max CAGR (half-Kelly) |
| `max_pos_pct` | 0.20 | 0.25-0.35 | Concentrated portfolio |
| `split_frac` | 0.50 | 0.30-0.50 | Take quick partial earlier (30-50% at first profit target) |
| `min_adr_pct` | 0.03 | 0.03-0.05 | Crypto is already volatile; slight increase |
| `min_consol_days` | 5 | 3-7 | Shorter consolidations in crypto |
| `min_price` | $5.00 | $0 | No penny filter for crypto |
| `min_vol` | 300,000 | 0 | Use min_adv instead for crypto |

**Key optimization parameters to sweep** (in priority order):
1. `split_frac` — this directly controls how fast you take profits, which determines capital velocity.
2. `risk_pct` — the Kelly lever. Higher = more CAGR but more drawdown.
3. `rs_pct` — selectivity of the universe. Too tight = too few trades. Too loose = too much noise.
4. `max_range_pct` — base quality. Too tight = misses valid crypto bases. Too loose = enters choppy ranges.
5. `min_consol_days` — the speed of the strategy. Lower = faster capital recycling. Higher = higher quality bases.

---

## Expected Performance Characteristics

Based on the research and parameter ranges above:

| Metric | Conservative | Target | Aggressive |
|--------|-------------|--------|------------|
| Win rate | 28-32% | 30-35% | 32-38% |
| Avg winner | 4-6% | 6-10% | 8-15% |
| Avg loser | 1.5-2.5% | 1.5-2% | 1-2% |
| Profit factor | 1.2-1.4 | 1.5-2.0 | 2.0-3.0 |
| Trades/year | 80-120 | 120-200 | 150-250 |
| Avg hold (days) | 5-10 | 3-7 | 2-5 |
| Sharpe ratio | 0.8-1.2 | 1.2-1.8 | 1.5-2.5 |
| Max drawdown | 20-30% | 30-45% | 40-60% |
| CAGR (in favorable regimes) | 30-60% | 60-120% | 100-200%+ |

**Important caveats:**
- "Aggressive" numbers require bull-market regimes and aggressive position sizing. They will not sustain across a full cycle.
- Max drawdown numbers are for the strategy alone. With regime filter, you avoid the worst (~70-90%) crypto bear market drawdowns by going to cash.
- These are backtested expectations. Live performance typically degrades 30-50% from backtest due to execution realities.
- The CAGR range is deliberately wide. The actual number depends heavily on how many setups the market offers and how long you're forced to sit in cash.

---

## Implementation Checklist

### Phase 1: Validate the Core Idea
1. Run the breakout setup on crypto training data with default params.
2. Compare to stock backtest results. If PF < 1.0 on crypto defaults: the setup may not transfer. Investigate.
3. Run a parameter sweep on `rs_pct`, `max_range_pct`, `min_consol_days` to find the broad optimum for crypto.
4. Run sub-period analysis (split crypto history into thirds) to check robustness.

### Phase 2: Optimize for Short Horizons
5. Sweep `split_frac` from 0.10 to 0.90 — find the partial exit percentage that maximizes CAGR (not Sharpe).
6. Backtest with different trailing stop methods: SMA(10) vs SMA(5) vs ATR(2x) trailing.
7. Add a time-stop parameter (max_hold_days) and sweep 3-15 days to find the optimal hold duration.
8. Run sensitivity tests: perturb each optimized param ±20% and check for stability.

### Phase 3: Aggressive Position Sizing
9. Sweep `risk_pct` from 0.005 to 0.03 to find the Kelly-optimal level.
10. Sweep `max_pos_pct` from 0.15 to 0.40 to find the concentration sweet spot.
11. Run Monte Carlo simulations on the equity curve at different sizing levels to quantify drawdown risk.

### Phase 4: Regime Filter Tuning
12. Compare regime filters: BTC > 200d SMA vs. BTC > 50d SMA vs. BTC 10 EMA > 20 EMA.
13. Measure improvement in Sharpe and max drawdown from each regime filter variant.
14. Optionally: combine regime filter with volatility overlay (reduce size when BTC ATR > threshold).

### Phase 5: Walk-Forward Validation
15. Run walk-forward analysis with shorter windows (12m IS / 3m OOS) appropriate for crypto.
16. Require WFE > 0.50.
17. If WFA passes: run on validation set (recent out-of-sample period).
18. If validation passes: one-shot holdout test.

---

## What "Success" Looks Like

A viable short-horizon crypto breakout strategy should:

- **Profit factor > 1.3** after all modeled costs (exchange fees + slippage).
- **Win rate 28-35%** with avg winner / avg loser ratio > 3:1.
- **Average hold 3-7 days** (capital cycling every week).
- **Sharpe > 1.0** net of costs.
- **Survive regime filter test**: profitable in at least 2 of 3 sub-periods.
- **WFE > 0.50** on walk-forward analysis.
- **The mechanism is explainable**: volatility contraction creates a supply/demand imbalance. When price breaks the consolidation high on volume, the exhaustion of sellers is confirmed. The stop is placed where the thesis is invalidated (below the base). The partial exit captures the "first predictable move." The trailing stop rides the remainder.

If these conditions are met, the strategy is viable for paper trading. Not for real capital — for paper trading. The Qullamaggie approach works because it's built on a genuine market mechanism (institutional accumulation → breakout), not curve-fitting. Our job is to verify that the mechanism transfers to crypto and that the numbers work after costs.

---

## Sources

### Academic Research
- [CTREND Factor — Cross Section of Cryptocurrency Returns (JFQA, 2025)](https://www.cambridge.org/core/journals/journal-of-financial-and-quantitative-analysis/article/trend-factor-for-the-cross-section-of-cryptocurrency-returns/4C1509ACBA33D5DCAF0AC24379148178)
- [Cryptocurrency as Investable Asset Class (arXiv, 2025)](https://arxiv.org/html/2510.14435v1)
- [Cryptocurrency Momentum Has (Not) Its Moments (Springer, 2025)](https://link.springer.com/article/10.1007/s11408-025-00474-9)
- [Time-Series and Cross-Sectional Momentum in Cryptocurrency (SSRN)](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=4675565)
- [Dynamic Time Series Momentum of Cryptocurrencies (ScienceDirect)](https://www.sciencedirect.com/science/article/abs/pii/S1062940821000590)
- [Alpha from Short-Term Signals (Alpha Architect, 2025)](https://alphaarchitect.com/alpha-from-short-term-signals/)
- [Kelly Criterion: Optimal Growth Rate and Rebalancing (Frontiers, 2020)](https://www.frontiersin.org/journals/applied-mathematics-and-statistics/articles/10.3389/fams.2020.577050/full)
- [Design and Analysis of Momentum Trading Strategies (arXiv)](https://arxiv.org/pdf/2101.01006)

### Practitioner Sources
- [US Stock Momentum Trading System for Retail (Cracking Markets)](https://www.crackingmarkets.com/us-stock-momentum-trading-system-for-retail-traders-deep-research/)
- [Qullamaggie Breakout Strategy (TradingView)](https://www.tradingview.com/script/cDCAPrd1-Qullamaggie-Breakout/)
- [SEPA Strategy Explained (QuantStrategy.io)](https://quantstrategy.io/blog/sepa-strategy-explained-mastering-trend-following-with-mark/)
- [Minervini SEPA Method (Finer Market Points)](https://www.finermarketpoints.com/post/what-is-mark-minervini-s-trading-strategy-the-complete-sepa-vcp-guide)
- [ATR Stop-Loss Strategies (LuxAlgo)](https://www.luxalgo.com/blog/5-atr-stop-loss-strategies-for-risk-control/)
- [ATR Trailing Stop for Crypto (Flipster)](https://flipster.io/blog/atr-stop-loss-strategy)
- [Systematic Crypto Strategies: Momentum, Mean Reversion, Volatility (Medium)](https://medium.com/@briplotnik/systematic-crypto-trading-strategies-momentum-mean-reversion-volatility-filtering-8d7da06d60ed)
- [Crypto Momentum Framework (Stoic.ai)](https://stoic.ai/blog/momentum-trading-indicators-strategy-expert-crypto-trading-guide/)
- [Altcoin Rotation — When and Why Altcoins Outperform (Block Scholes)](https://www.blockscholes.com/research/bybit-x-block-scholes-the-altcoin-rotation-why-and-when-altcoins-outperform-bitcoin)
