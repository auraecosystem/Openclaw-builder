# Qullamaggie (Kristjan Kullamagi) Setup Index

Primary sources: qullamaggie.com blog posts, FAQ, Chat With Traders interview transcripts, Twitch stream notes.
Direct quotes are marked with block quotes.
These rules capture the high-confidence, repeated parts of the public playbook. Qullamaggie's real execution is still discretionary, so treat the checklists as the hard core of the method rather than a promise of perfect mechanical replication.

This file is now an overview and index. Detailed setup rules live in separate references:

- [Continuation Breakout (VCP / Flag)](./qullamaggie-breakout.md)
- [Episodic Pivot (EP)](./qullamaggie-episodic-pivot.md)
- [Parabolic Short](./qullamaggie-parabolic-short.md)

---

## Background

- Swedish trader based in Stockholm. Started in 2011 at age 23.
- First profitable year: 2013.
- Achieved financial independence in 2017. Streamed publicly from 2017 to 2019.

**Verified account milestones (self-reported, shown on stream):**

| Date          | Account Value         |
| ------------- | --------------------- |
| May 2013      | $9,100 (low point)    |
| January 2018  | $1,400,000            |
| August 2020   | $17,000,000           |
| December 2020 | $34,000,000           |
| February 2021 | $59,000,000           |
| March 2021    | $82,000,000           |
| 2024          | ~$100-130M (reported) |

- CAGR 2013-2019: 268% compounded (self-reported FAQ).
- Ranked 15th highest income earner in Sweden in 2021 (secondary source).

**Win rates:**

- 2019: 25%
- 2020: 35%

> "My win rate last year (2020), and I had insane returns, was only 35%. The year before that in 2019 it was only 25%."

**Profit distribution:** roughly 10-20% of trades generate most of the profits. The rest mostly scratch or stop out.

**Drawdowns:** worst historical drawdown cited here is 50% in 2014. Target is usually to contain drawdowns to 15-20%.

---

## Core Framework

Qullamaggie's playbook is three distinct setups sharing one broad operating framework:

1. **Continuation Breakout**
   Long momentum continuation from a tight base in a leading stock. See [qullamaggie-breakout.md](./qullamaggie-breakout.md).

2. **Episodic Pivot**
   Long entry after a catalyst-driven revaluation gap on huge volume. See [qullamaggie-episodic-pivot.md](./qullamaggie-episodic-pivot.md).

3. **Parabolic Short**
   Short mean reversion after an unsustainably steep run-up. See [qullamaggie-parabolic-short.md](./qullamaggie-parabolic-short.md).

---

## Daily Routine And Platform Setup

- 8-9am: wakes and scans Twitter
- 10am-12pm: workout
- 1pm: watches pre-market movers and checks portfolio
- 2-2:30pm during earnings season: checks for earnings gaps
- 3pm Stockholm / US pre-market: prepares for stream

**Platforms:**

- TC2000 for charting and scanning
- Sterling Trader Pro for execution
- Interactive Brokers as secondary broker
- eSignal for intraday open context
- MarketSmith for EPS/revenue data
- KoyFin for forward estimates
- Seeking Alpha for research
- TheFly for news and earnings alerts

---

## Universe And Scanning

Starts with ~6,000+ stocks and narrows to a smaller actionable list using scans in TC2000.

**Scan 1 - Continuation base**

- Up at least 25% in the past month
- Current week volume at least 50% lower than prior week
- Trading within roughly 2% above or below the 10-day SMA

**Scan 2 - Episodic pivot**

- One-day move greater than 7.5%
- Dollar volume at least $100M on the gap day

**Scan 3 - Momentum leaders**

- Top 1-2% performers across 1-month, 3-month, and 6-month windows

**Scan 4 - ADR / trend intensity**

- Dollar volume at least $1,500,000
- ADR filter to ensure sufficient movement
- EMA(7) more than 8% above EMA(65)

**Relative strength definition**

- Top 1-2% across 1-month, 3-month, 6-month, 12-month, and 18-month performance windows
- Leaders hold higher lows during market weakness, break out before peers, and resist sector downdrafts

**Sector weight**

- He applies the O'Neil idea that industry group strength matters materially
- Avoid lagging sectors and prefer the strongest themes

---

## General Universe Filters

- No ETFs or funds
- Price above $5
- 20-day average daily volume above 300,000 shares
- Exclude binary, pre-revenue biotech situations

## Non-Negotiables

- Buy strength, not weakness. He does not want broken stocks or anticipatory entries.
- Prefer the top 1-2% relative strength names, not average names in average groups.
- Do not chase names already more than roughly 1 ADR extended from the base on entry day.
- Keep each position liquid enough that a single order does not exceed 1% of average daily dollar volume.
- Use hard invalidation levels and market-stop discipline.
- Reduce size and activity materially in weak tape conditions.

---

## Shared Risk And Execution Rules

These rules apply broadly across the playbook unless a setup file overrides them.

**Risk per trade**

- Normally 0.25-1% of account equity
- Rarely above 1%
- Exceptional parabolic short cases have gone higher, but those are explicitly rare exceptions

**Sizing formula**

```text
shares = (account_equity * risk_pct) / abs(entry_price - stop_price)
```

**Position size**

- Roughly 5-25% of account
- Typical position is 10-15%

**Overnight concentration**

- Never hold more than 30% of account in one name overnight

**Position count**

- Usually 15-20 positions
- Can expand to 20-25 in strong bull runs

**Liquidity cap**

- Never exceed 1% of average daily dollar volume in a single order

**Stop execution**

- Uses market stops, not limit stops

> "If I get stopped out, I get stopped out. I don't think, I just get out."

**ADR formula**

- Average of `(high - low) / low * 100` over the prior 20 sessions

## Shared Trade Management Sequence

This is the repeatable management template across the long setups unless the setup file says otherwise:

1. Enter only on confirmation.
2. Set the initial stop immediately from the setup structure.
3. Confirm stop width is normally no greater than 1 ADR.
4. Size the trade from the stop distance and risk budget.
5. If the trade works, sell roughly 20-25% after 3-5 days if profitable.
6. Move the stop on the remainder to breakeven after that first partial.
7. Trail the remainder with the 10-day or 20-day moving average.
8. Exit the runner only on a close below the chosen moving average, unless a catastrophic hard-stop event forces an earlier exit.

---

## Market Regime Filter

**Primary regime check**

- QQQ daily 10 EMA versus 20 EMA
- Long setups are favored when the 10 EMA is above the 20 EMA and both slope up

> "Works approximately 80% of the time."

**Best breakout environment**

- Just as the market is emerging from a pullback or correction

**In weak markets**

- Reduce size
- Reduce activity
- Prefer parabolic shorts or stay flat

## No-Trade Conditions

Skip the trade if any of these are true:

- The stock is not a true leader on relative strength.
- The sector is lagging or structurally weak.
- The setup would require chasing far beyond the base.
- The stop would be materially wider than 1 ADR without exceptional justification.
- The stock is too illiquid for disciplined sizing.
- The setup sits directly in front of earnings or another binary event without sufficient profit cushion.
- You are stacking too many correlated positions in the same theme.
- The broad market regime is hostile and the setup is not an EP or a parabolic short.

---

## Process And Psychology

**In drawdown**

- Reduce size
- Trade less
- Observe more

**Earnings risk**

- Avoid holding through binary earnings unless there is meaningful profit cushion

**Correlation risk**

- Avoid stacking multiple similar names in the same sector theme at once

**Study method**

- Built a large chart study database across decades of winners
- Emphasis is on pattern repetition and contextual variation, not just textbook definitions

> "Spent THOUSANDS of hours looking at big winning stocks over the past 100 years and figured out the patterns that happen over and over again."

**On psychology**

- His claim is that conviction comes more from setup mastery than from generic mindset coaching

## Execution Checklist

Use this as the pre-trade gate. If one item fails, skip.

1. Is the stock a true relative-strength leader?
2. Is the sector also acting well, or at least not fighting the trade?
3. Does the setup match one of the three playbooks exactly?
4. Is the catalyst / pattern quality good enough to justify attention?
5. Is the stop placement obvious and within the normal ADR limit?
6. Does position size remain within portfolio heat, overnight cap, and liquidity cap?
7. Are you taking the trade because the pattern is valid, not because you want action?
8. Is the market regime supportive for this setup type?

---

## Quick Setup Map

| Setup                 | Core Idea                                | Long/Short | Trigger                                      |
| --------------------- | ---------------------------------------- | ---------- | -------------------------------------------- |
| Continuation Breakout | Strong leader pauses, then resumes       | Long       | Tight base breaks out on volume              |
| Episodic Pivot        | Surprise catalyst forces revaluation     | Long       | Gap and expansion on massive volume          |
| Parabolic Short       | Unsustainable vertical move mean-reverts | Short      | First red day, OR low break, or VWAP failure |

Use the setup-specific files for entry timing, stop placement, hold rules, and setup-specific filters:

- [qullamaggie-breakout.md](./qullamaggie-breakout.md)
- [qullamaggie-episodic-pivot.md](./qullamaggie-episodic-pivot.md)
- [qullamaggie-parabolic-short.md](./qullamaggie-parabolic-short.md)

---

## Recommended Reading

- _Reminiscences of a Stock Operator_ by Lefevre
- _How to Make Money in Stocks_ by O'Neil
- _Market Wizards_ series by Schwager
- _How I Made $2,000,000 in the Stock Market_ by Darvas
- _Trade Like a Stock Market Wizard_ by Minervini
- _Trade Like a Champion_ by Minervini
