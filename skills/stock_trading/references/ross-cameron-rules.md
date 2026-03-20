# Ross Cameron (Warrior Trading) Setup Index

Primary sources: warriortrading.com strategy pages, Ross Cameron verified earnings reports, FTC case documentation.
This approach is intraday day trading, not swing trading. Positions are normally opened and closed the same day.
These rules capture the recurring, explicit parts of the public Warrior Trading playbook. Ross Cameron's actual execution still includes discretion, tape-reading, and market-regime judgment, so this should be read as the hard-rule core rather than a guarantee of fully mechanical replication.

This file is now an overview and index. Detailed setup rules live in separate references:

- [Gap and Go](./ross-cameron-gap-and-go.md)
- [Bull Flag / First Pullback](./ross-cameron-bull-flag.md)
- [Flat Top Breakout](./ross-cameron-flat-top-breakout.md)
- [ABCD Pattern](./ross-cameron-abcd.md)

---

## Background And Caveats

- Founded Warrior Trading in 2012.
- Small account challenge began on January 1, 2017 with $583.15.
- Broker statements have been described as independently reviewed.

**Reported cumulative P&L milestones:**

| Period             | Milestone                      |
| ------------------ | ------------------------------ |
| Day 44 (~Feb 2017) | Account broke $100,000         |
| May 2019           | Crossed $1,000,000 cumulative  |
| 2020               | Roughly $4.5M net              |
| 2022               | Crossed $10,000,000 cumulative |
| 2023               | Roughly $306,548               |
| End 2024           | $12,609,991 cumulative         |

**Important caveat**

- The FTC reached a 2022 settlement with Warrior Trading over earnings claims and student outcomes.
- Cameron's own reported results and the average student outcome should not be treated as comparable.
- Market regime matters enormously for this style; 2020 was unusually favorable for small-cap momentum.

---

## Core Framework

Ross Cameron's playbook is a small-cap momentum day-trading framework built around a few recurring premises:

1. Trade only stocks already making abnormal moves.
2. Require real catalyst plus unusual volume.
3. Focus on low-float names because they can move the fastest.
4. Take entries at intraday confirmation points, not on anticipation.
5. Keep risk tight and hold times short.

This is best understood as four related setups sharing one scanner-and-risk framework:

1. **Gap and Go**
   Opening-range momentum after a pre-market gap on news. See [ross-cameron-gap-and-go.md](./ross-cameron-gap-and-go.md).

2. **Bull Flag / First Pullback**
   Continuation after the first orderly consolidation in a strong mover. See [ross-cameron-bull-flag.md](./ross-cameron-bull-flag.md).

3. **Flat Top Breakout**
   Repeated tests of one resistance level, then expansion through it. See [ross-cameron-flat-top-breakout.md](./ross-cameron-flat-top-breakout.md).

4. **ABCD Pattern**
   Impulse, pullback, then continuation through the prior pivot. See [ross-cameron-abcd.md](./ross-cameron-abcd.md).

---

## Stock Selection: The 5 Pillars

Every candidate should satisfy the core small-cap momentum filters.

**Pillar 1 - Relative volume**

- Heavy relative volume is required
- In the stricter Warrior framing, this is often 5x or more versus normal

**Pillar 2 - Daily percent change**

- He wants stocks already moving materially, often 10% or more

**Pillar 3 - News catalyst**

- Earnings, FDA, partnership, government contract, major PR, or another concrete catalyst
- No catalyst usually means no trade

**Pillar 4 - Price range**

- Common focus range is roughly $1-$20
- Sweet spot is often around $5-$10

**Pillar 5 - Float**

- Low float is critical
- Under 10M is ideal
- Under 5M is especially explosive

## Gapper Quality Ratings

| Grade | Float     | Short Interest | Catalyst          | Pre-market Volume    |
| ----- | --------- | -------------- | ----------------- | -------------------- |
| A     | Under 20M | Over 10%       | Strong            | Over 150,000 shares  |
| B     | 50-100M   | 5-10%          | Weaker            | Under 100,000 shares |
| C     | Over 100M | Under 5%       | Weak or small gap | Avoid                |

**Gap threshold**

- Gaps above 4% are the minimum acceptable range for Gap and Go work
- Smaller gaps are more likely to fill and are less attractive

---

## Scanner Infrastructure

**Pre-market gap scanner**

- Builds the morning watchlist before the open

**HOD momentum scanner**

- Looks for fresh intraday highs and continuation
- Often described as one of the strongest live scanners in this framework

**Surging volume / momentum scans**

- Used to find the one stock that is truly in play right now

**Focus rule**

- He tries to identify the single most obvious stock of the day, not spread attention across too many names

## Typical Morning Routine

- Around 7:00-7:15am ET: review leading pre-market gappers
- Around 9:00am ET: narrow to the final watchlist
- Before 9:30am ET: mark pre-market highs, lows, and obvious flags

---

## Shared Risk And Execution Rules

These apply across the Ross Cameron playbook unless a setup file says otherwise.

**Reward-to-risk check**

- Minimum 2:1 reward to risk before entry

**Position sizing**

```text
shares = acceptable_dollar_loss / stop_distance_per_share
```

- Tight stop means larger size is possible
- Wide stop means smaller size
- Often uses a starter size first, then adds on confirmation

**Daily max loss**

- Uses a personal circuit breaker
- Once max loss is hit, trading stops for the day

**Practical limit from the source material**

- Roughly $2,000 max loss per account, about $4,000 total across two accounts in the cited framework

**Scaling out**

- Often sells half at the first target
- Then moves the stop on the remainder to breakeven

**Exit logic**

- First red candle can be the exit if no partial has been taken yet
- If the breakout fails to follow through quickly, he exits fast

**Execution style**

- Very short average hold times
- Heavy focus on Level 2 and tape confirmation
- Uses mental stops rather than always placing hard stops in the market

## No-Trade Conditions

Skip the trade if any of these are true:

- No clear catalyst
- Relative volume is mediocre
- The stock is not up enough to be truly in play
- Float is too large for the expected percentage move
- Spread and liquidity are too sloppy
- Setup appears after the best morning window without enough quality to compensate
- Reward-to-risk is under 2:1
- You are trying to trade multiple mediocre names instead of the obvious leader

---

## Intraday Timing

**Primary trading window**

- 9:30am to 11:30am ET

| Window        | Main Use                                 |
| ------------- | ---------------------------------------- |
| 9:30-10:00am  | Gap and Go, opening-range momentum       |
| 9:30-11:00am  | Core 1-minute momentum setups            |
| 11:00-11:30am | More selective continuation trades       |
| After 11:30am | Mostly 5-minute chart only, fewer trades |

The edge is concentrated in the morning when small-cap momentum is strongest.

## Discipline Checklist

1. Does the stock meet all 5 Pillars?
2. Is it the obvious leader, not just a random mover?
3. Is there a real setup, not just green candles?
4. Is the entry trigger explicit and visible?
5. Is the stop level obvious before entry?
6. Does the first target still give at least 2:1 reward to risk?
7. Are you inside the time window where this setup actually works?
8. If the trade fails immediately, are you prepared to cut it without debate?

---

## Quick Setup Map

| Setup                      | Core Idea                               | Typical Time    | Trigger                                |
| -------------------------- | --------------------------------------- | --------------- | -------------------------------------- |
| Gap and Go                 | Pre-market gap continues after open     | 9:30-10:00      | ORB or pre-market high break           |
| Bull Flag / First Pullback | Strong mover consolidates, then resumes | Morning session | First new high after pullback          |
| Flat Top Breakout          | Repeated resistance finally gives way   | Morning session | Break above flat ceiling               |
| ABCD                       | Impulse, pullback, then continuation    | Morning session | Break above B / C-D continuation pivot |

Use the setup-specific files for the actual entry and stop mechanics:

- [ross-cameron-gap-and-go.md](./ross-cameron-gap-and-go.md)
- [ross-cameron-bull-flag.md](./ross-cameron-bull-flag.md)
- [ross-cameron-flat-top-breakout.md](./ross-cameron-flat-top-breakout.md)
- [ross-cameron-abcd.md](./ross-cameron-abcd.md)
