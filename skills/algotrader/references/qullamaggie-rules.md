# Qullamaggie (Kristjan Kullamägi) — Swing Trading Rules

Primary sources: qullamaggie.com blog posts, FAQ, Chat With Traders interview transcripts, Twitch stream notes.
Direct quotes marked with block-quote formatting.

---

## Background & Verified Performance

- Swedish trader based in Stockholm. Started 2011 at age 23. Blew up account 3-4 times (2011-2012, starting capital $2,000-5,000 each time).
- First profitable year: 2013. Account low point that year: $9,100.
- Achieved financial independence 2017. Began streaming publicly 2017-2019.

**Verified account milestones (self-reported, shown on stream):**

| Date | Account Value |
|---|---|
| May 2013 | $9,100 (low point) |
| January 2018 | $1,400,000 |
| August 2020 | $17,000,000 |
| December 2020 | $34,000,000 |
| February 2021 | $59,000,000 |
| March 2021 | $82,000,000 |
| 2024 | ~$100-130M (reported) |

- Ranked 15th highest income earner in Sweden in 2021 (secondary source).
- CAGR 2013-2019: 268% compounded (self-reported FAQ).

**Win rates:**
- 2019: 25%
- 2020: 35%

> "My win rate last year (2020), and I had insane returns, was only 35%. The year before that in 2019 it was only 25%. You have to get used to getting stopped out a lot. Many times the same day and many times within a few minutes. That's the name of the game."

**Profit distribution:** ~10-20% of trades generate most of the profit. The rest roughly break even. He describes himself as a "home run trader," not a high-win-rate trader.

**Drawdowns:** Worst: 50% in 2014. Target: contain drawdowns at 15-20%; happens "a few times per year."

---

## Daily Routine & Platform Setup

- 8-9am: Wakes, scans Twitter in bed
- 10am-12pm: Works out (intermittent fasting, no breakfast)
- 1pm: Watches pre-market movers, checks portfolio
- 2-2:30pm (earnings season): Checks for earnings gaps
- 3pm Stockholm (US pre-market): Sits at computer, prepares for stream

**Platforms:**
- TC2000 — charting and scanning
- Sterling Trader Pro — execution (primary)
- Interactive Brokers — second broker
- eSignal — intraday near open
- MarketSmith — EPS/revenue data
- KoyFin — future estimates
- Seeking Alpha — research
- TheFly — news and earnings alerts
- Five monitors

---

## Universe / Scanning

Starts with ~6,000+ stocks; filters to ~200 names using four scan criteria in TC2000.

**Scan 1 — Continuation base:**
- Up ≥25% in the past month
- Current week volume ≥50% lower than prior week
- Trading within ~2% above or below the 10-day SMA

**Scan 2 — Episodic pivot:**
- 1-day price move >7.5%
- Dollar volume ≥$100M on gap day

**Scan 3 — Momentum leaders:**
- Top 1-2% performers across 1-month, 3-month, 6-month timeframes

**Scan 4 — ADR/trend intensity:**
- Dollar volume ≥$1,500,000 minimum
- ADR (20-day) filter (threshold not published; stocks must move enough to be interesting)
- EMA(7) more than 8% above EMA(65)

**Relative strength definition:** Stocks in the top 1-2% of performers across 1-month, 3-month, 6-month, 12-month, and 18-month timeframes. Leaders exhibit higher lows during market retests, break out before peers, and hold up when their sector declines.

**Sector weight (O'Neil principle he applies):** 37% of a stock's price movement is tied to its industry group; 12% from broader sector. He avoids lagging sectors and favors leading themes.

---

## General Universe Filters (All Setups)

- No ETFs, no funds
- Price >$5 (avoid penny stocks)
- Average daily volume (20-day) >300,000 shares
- Exclude pre-revenue biotech and binary-event stocks

---

## Setup 1: Continuation Breakout (VCP / Flag)

Bread-and-butter setup. Stock makes a large initial move, consolidates in an orderly base, breaks out to continue higher.

### Additional Universe Filters

- Top 1-2% relative strength over 1, 3, 6 months vs. broad market
- Price within 25% of 52-week high (near highs, not broken stocks)

### Pattern Criteria

**VCP (Volatility Contraction Pattern):**
- ≥2 contractions in price range, each tighter than the prior
- Duration: 2-8 weeks
- Volume declines during base — current week volume ≥50% lower than prior week

**Flag:**
- Prior impulse move ≥20% in 1-4 weeks
- Shallow pullback: <50% retrace of the impulse
- Declining volume during consolidation
- Duration: 5-25 days

**Both:** Current consolidation range <15% (tight base). Stock is "surfing" the 10-day and 20-day MAs with higher lows.

> "I find it hard to buy pullbacks, because the best breakouts don't pullback."
> "I buy breakouts as they break out on the daily chart — I don't anticipate breakouts."

**Avoid:** Stocks that have already moved more than their ATR from the base on the entry day.

### Entry (Exact Timing Rules)

Entry method depends on *when* the breakout occurs:

| Time | Entry |
|---|---|
| Gap up at open | Buy 1-minute Opening Range High (ORH) |
| Breaks within first 5 minutes | Buy 5-minute ORH |
| Breaks 9:35-10:00am ET | Buy hourly ORH |
| After 10:00am ET | Enter directly on daily chart breakout |

- Can add on 5-minute or 60-minute highs if initial 1-minute entry is missed
- Enter full position at once, aggressively: > "I buy my full position at once, very aggressively. I just want to get in and get out in a few seconds."
- Volume on breakout: must be >1.5× 20-day average (confirms real demand)

### Stop Loss (Exact)

- Initial stop: Low of day (LOD) on entry day
- Constraint: stop must not be wider than 1× ADR (20-day average daily range in %)
- Maximum: 1× ADR; occasionally 1.5× ADR in exceptional cases
- Always uses market stops, never limit stops: > "If I get stopped out, I get stopped out. I don't think, I just get out."
- Overriding stops: > "Very rarely, but sometimes yes. If you made less than a few million in the markets you should never do that."

**ADR formula:** Average of `(high - low) / low * 100` over past 20 trading sessions.

### Partial Profits (Exact)

- Sell 20-25% of position (or 1/3 to 1/2) after 3-5 days if profitable
- > "The first move is the most predictable part of the move."
- After first partial: move remaining stop to break-even

### Trailing Stop (Exact)

- Trail remaining position with 10-day MA (faster-moving stocks) or 20-day MA (slower movers)
- He currently favors the 20-day MA
- Exit **only on a close below** the chosen MA — intraday violations are acceptable; do NOT exit intraday
- Maintains a hard stop for catastrophic overnight gaps

### Adding to Winners

Treats each addition as an independent trade with its own stop. Will re-buy at higher prices if the stock sets up a new valid base after weeks of consolidation.

Example from CVNA: three separate stops at $13.32, $15.47, and $18.08 for three entry tranches.

---

## Setup 2: Episodic Pivot (EP)

A gap-up on unexpected news that launches a multi-month trend. Stock was neglected; a catalyst forces a fundamental revaluation.

### What Qualifies as a Catalyst

- Political/regulatory changes (e.g., bank and prison stocks post-election)
- FDA/biotech approvals
- Major contracts and partnerships
- Earnings surprise with upward guidance revision (triple-digit YoY earnings+sales growth is ideal)
- Sector-wide catalysts in hot sectors

> "News that catches the market off guard and forces a revaluation of the stock."

For earnings EPs: "Triple digit year-over-year earnings and sales growth" ideally; significant analyst beat; "big guidance higher."

### Setup Criteria (Exact)

- Gap ≥10% from prior close on catalyst day
- EP scan: 1-day move >7.5%; dollar volume ≥$100M on gap day
- Stock should NOT have rallied in the prior 3-6 months — surprise must be genuinely unexpected
- Ideal: stock in sideways consolidation for 3-6 months before the gap
- > "The failure rate is higher and the move probably won't be as big" if the stock already had a recent prior EP

### Volume Requirement (Exact)

> "Massive volume near the open. Ideally the stock should trade the average daily volume the first 15-20 minutes or even quicker."

Seeing ADV traded in 15-20 minutes is the primary confirmation filter for institutional involvement.

### Entry Timing (Exact)

- Most successful entries: 3-10 minutes after open (confirmed in June 2023 stream notes)
- Primary entry: 1-minute ORH
- Add on 5-minute highs for additional confirmation and size
- Can use 5-minute or 60-minute highs if 1-minute entry is missed
- Can add throughout the day if price action remains favorable

### Stop Loss

- Stop: low of the gap day (LOD)
- If entry is next-day open, stop is still the gap day's low
- Maximum: 1× ADR (occasionally 1.5× ADR)

### Hold Duration & Market Conditions

- EPs can lead to multi-week to multi-month moves; trail with 10-day or 20-day MA
- More resilient across market conditions than standard breakouts
- Will trade EPs even in bear markets with adjusted size/expectations
- During earnings seasons: 20-25 EP opportunities normally; 50-100 after a correction when expectations are low

> "Takes 4-8 earnings seasons (1-2 years) to get good at this setup."

Best risk/reward of his three setups: tight stops vs. multi-month potential.

---

## Setup 3: Parabolic Short

Stocks that have run up parabolically snap back to their moving averages. Timing the "first red day" or VWAP failure is the edge.

### Definition of "Parabolic" (Exact)

- Large-cap (price >$50): up ≥50% in last 10 trading days
- Small-cap (price ≤$50): up ≥300% in last 10 trading days
- ≥3-5 consecutive green days (close > open) leading to entry
- General filters still apply

### Entry Methods (Three Valid Entries)

1. Short at the opening range low using 1-minute or 5-minute candles on the first red day
2. Wait for the first red 5-minute candle if momentum continues at open ("first red candle")
3. Wait for stock to decline, bounce to VWAP, fail at VWAP (marked by a red candle at VWAP), then short — stop is a reclaim of VWAP

**Timing:** Win rate is described as 80-90% on "day 3 or 4" after the initial surge. Do NOT short day 1 or 2.

> "Don't be too early. WAIT for the right setup."

### Stop Loss

- Standard entries: stop at the day's high
- VWAP failure entry: stop is a reclaim of VWAP

### Profit Targets

- Target: 10-day SMA and 20-day SMA — "that's where these stocks usually bounce"
- Natural magnet levels for parabolic mean reversion
- R:R typically 5-10× on well-timed entries

### Qullamaggie's Honest Take on Shorts

> "Guys, don't short. You don't ever have to short. You can make, you can average 100, 200 percent per year just trading the long side. You never have to use margin. You never have to short, but what you have to do is to be patient. Wait for your spots."

He has risked up to 4% of account total on exceptional parabolic short trades (APT COVID 2020, MRNA December 2020) — rare exceptions to his normal 0.25-1% risk per trade.

---

## Position Sizing (Exact Rules)

**Standard risk per trade:** 0.25-1% of account. Rarely more than 1%.

**Formula:**
```
shares = (account_equity × risk_pct) / abs(entry_price - stop_price)
```

Where `risk_pct` is 0.0025 to 0.01 (0.25%-1%).

**Position size as % of account:**
- General range: 5-25%
- Most positions: 10-15%
- Factors: stock liquidity, conviction, overall riskiness

**Overnight cap:** Never hold more than 30% of account in any single name overnight. Intraday can exceed 30%.

**Position count:** Typically 15-20 simultaneously; up to 20-25 in strong bull runs. >30 open positions historically precedes a market pullback.

**Liquidity constraint:** Never enter more than 1% of average daily dollar volume in a single order.

**Scaling with growth:** Dollar position size scales proportionally as account grows. Always thinks in percentages, never absolute dollar amounts.

> "Margin is something you have to deserve. It's not a privilege."

---

## Market Regime Filter (Exact)

**Primary indicator (June 2023 stream):**
- QQQ daily chart: 10 EMA vs. 20 EMA
- Green zone (take longs): 10 EMA above 20 EMA, both sloping upward
- Cash zone: 10 EMA below 20 EMA
- > "Works approximately 80% of the time."

**Best timing for breakouts:** "Just as the market is coming out of a pullback or correction."

**Corrections as filters:** Stocks showing "tennis ball" action — bouncing back hard while the market is still weak — are the next leaders.

**In downtrends:** Dramatically reduce activity and size; Setup 3 (parabolic shorts) only, or flat. > "When the market is no good, you want to limit your exposure & action in order to limit the potential damages you do to your portfolio."

**Borrowed framing from Minervini:** "Easy dollar" environments (take full size, be aggressive) vs. "Hard penny" environments (cut size, be selective or flat).

---

## Risk Management

**In drawdown/losing streaks:** Reduce position size, trade less, observe more.

**Earnings risk:** > "I really don't like to gamble over earnings...without a big profit padding." Exits before binary events unless he has meaningful profit cushion.

**Correlation risk:** Avoids entering multiple similar sector trades simultaneously. During crises, "everything is so correlated" and limiting correlated exposure reduces portfolio-level risk.

**Multiple positions:** Each position gets its own stop, sized independently. Tranches in the same stock get separate stops.

---

## Psychology & Process

> "Throw away all your trading psychology books. Psychology was never the problem to begin with."

His view: mastery of setup variations — knowing exactly when setups work and when they don't — builds real conviction without emotional management techniques.

**Study method:** Built an Evernote database of thousands of historical chart examples (pre-move, breakout, post-move on daily and intraday timeframes). Studies patterns dating back to the 19th century. Estimated 10,000+ hours of study.

> "Spent THOUSANDS of hours looking at big winning stocks over the past 100 years and figured out the patterns that happen over and over again."

**On options and CFDs:**
> "It's like poison. Avoid. Don't touch unless you've made a few million trading stocks... If the fail rate for stock traders is 95%+, the fail rate for options and CFD traders is 99%."

**His acknowledged weaknesses (self-reported):** Overtrading; selling too early in parabolic moves; lacking patience; not following his own sell rules.

**Approximate annual trade count:** ~1,200 trades/year (~5 per trading day).

---

## Quick Reference: All Rules in One Place

| Rule | Value |
|---|---|
| Risk per trade | 0.25-1% of account (max 4% on rare exceptional parabolic shorts) |
| Position size | 5-25%; typical 10-15% |
| Overnight limit | 30% max in any single name |
| Max positions | 15-20 normal; up to 25 in bull runs |
| Stop type | Always market stop (never limit) |
| Stop width | LOD; max 1× ADR (rarely 1.5×) |
| Partial profit | Sell 20-25% (or 1/3-1/2) after 3-5 days |
| After partial | Move stop to break-even |
| Trailing stop | 10-day MA (fast) or 20-day MA (slow); exit on close only |
| Breakout entry | 1-min ORH if gap; 5-min ORH if early break; hourly ORH if 9:35-10am; daily close otherwise |
| EP entry | 3-10 min after open; ADV must trade in 15-20 min |
| Parabolic short entry | ORLow, first red candle, or VWAP failure on day 3-4 |
| Parabolic short target | 10-day MA or 20-day MA |
| Market regime | Long only when QQQ's 10 EMA > 20 EMA |
| Relative strength | Top 1-2% across 1/3/6/12/18-month periods |
| Liquidity cap | Never exceed 1% of avg daily dollar volume per order |
| ADR formula | Average of `(high-low)/low*100` over 20 sessions |

---

## Recommended Reading (His List)

- *Reminiscences of a Stock Operator* — Lefèvre
- *How to Make Money in Stocks* — O'Neil
- *Market Wizards* series — Schwager
- *How I Made $2,000,000 in the Stock Market* — Darvas
- *Trade Like a Stock Market Wizard* — Minervini
- *Trade Like a Champion* — Minervini
