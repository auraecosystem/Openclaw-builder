# Qullamaggie Continuation Breakout (VCP / Flag)

Bread-and-butter long setup. A strong stock makes an initial move, consolidates in an orderly base, then breaks out to continue higher.

## Additional Universe Filters

- Top 1-2% relative strength over 1, 3, and 6 months
- Price within 25% of the 52-week high

## Exact Setup Definition

This is a continuation setup, not a reversal setup. The stock should already have made a meaningful prior move, then tighten in an orderly way near highs before resuming.

## Scanning For Candidates

Use the breakout scan to build a broad watchlist, then rank the results by quality:

- Up at least 25% in the past month
- Current week volume at least 50% lower than the prior week
- Trading within roughly 2% above or below the 10-day SMA
- Top 1-2% relative strength across the main lookback windows
- Price within 25% of the 52-week high

Exclude immediately:

- ETFs and funds in stock-only scans; keep them in a separate optional universe if you intentionally trade ETF breakouts
- Stocks under $5
- 20-day average daily volume under 300,000 shares
- Pre-revenue biotech and other binary-event names
- Lagging sectors
- Bases that are already too wide, loose, or extended

## Improving TWS-Based Scanning For Fresh VCP Entries

These notes are for scanner design, not a claim that every numeric threshold below is core Qullamaggie doctrine. Keep the hard rules source-backed and treat the mechanical thresholds as tunable implementation heuristics.

**Core design**

- Use TWS only for broad discovery. IBKR scanner output is best treated as "who is moving," while the local filter layer decides "who is still buyable."
- Run a union of several scan codes and locations each cycle to work around the single-scan 50-row cap. `TOP_PERC_GAIN`, `HIGH_VS_52W_HL`, `HIGH_VS_13W_HL`, `MOST_ACTIVE`, and `HOT_BY_VOLUME` are reasonable inputs if they are available in your scanner metadata.
- Deduplicate names across scans and use recurrence frequency as one score input, not as a standalone buy signal.
- Take staggered snapshots through the session and give extra weight to names that keep showing up. Treat persistence as a quality bonus, not a mandatory filter.

**Hard filters that survived validation**

- Apply a freshness / anti-chase rule. Reject names already more than roughly 1 ADR above the pivot or otherwise clearly extended beyond the breakout area.
- Only promote names that are still breakout-ready, not spent. A valid candidate should still be near the pivot, tight, and showing evidence of a fresh base rather than just a prior breakout that already ran.
- Keep the relative-strength requirement. The strongest candidates should still rank in the top tier across the main 1-month, 3-month, and 6-month lookback windows.
- Keep sector confirmation. Use sector or industry ETF strength as a practical proxy and down-rank setups where the group is lagging.
- Keep event-risk guardrails. Skip fresh-entry candidates sitting directly in front of earnings or another obvious binary event unless the strategy explicitly allows that risk.
- Exclude ETNs, leveraged products, obvious SPAC junk, and similar low-quality instruments by default. Binary biotech should be skipped or explicitly opt-in. Plain ETFs are optional and should live in a separate universe if included.

**Mechanical proxies that are reasonable but tunable**

- Detect a fresh-base reset after a prior breakout with range-compression checks, controlled pullback depth, and a ban on names that are still in vertical extension mode. A 10-day versus 20-day compression comparison is a reasonable proxy, not a canonical rule.
- Score contraction sequencing. Favor at least 2 contraction legs with smaller swing amplitude and penalize the "one large pullback and one bounce" look.
- Require volume dry-up into the pivot. Weekly volume contraction remains the cleaner signal, but 5-day or 10-day average volume versus the prior period is a workable automation proxy.
- Prefer price to remain in a narrow band around the pivot, but do not hard-code an exact band unless it has been tested on your own output.
- ATR compression can be used as another tightening proxy, for example by checking whether shorter-lookback ATR is below longer-lookback ATR. Treat this as an optional overlay, not a standalone rule.
- Keep the documented 300,000-share 20-day liquidity floor as the broad minimum. Stricter tiers such as 500,000 to 1,000,000 shares are execution-quality overlays for larger size, not required doctrine.

**Ranking and workflow**

- Rank candidates on tightness, contraction quality, proximity to pivot, anti-chase, volume behavior, trend alignment, and group strength. Exact weights and top-decile cutoffs are implementation choices.
- Maintain lifecycle states such as `candidate -> actionable -> triggered -> spent` so already-expanded breakouts do not repeatedly resurface as fresh entries.
- For the top-ranked names, render annotated charts and do a manual validation pass before promotion. The pattern is still discretionary even when the scanner is systematic.
- Recalibrate thresholds against recent outputs and prioritize precision over recall. The purpose of the scanner is to surface a small number of high-quality watchlist names, not to maximize raw hit count.

## Free Scanner Setups

Public Qullamaggie guidance is still broad rather than "one perfect scanner." The repeated idea is to scan the strongest stocks first, then manually review charts for fresh continuation structure.

**Qullamaggie core scan philosophy**

- Scan the top 1-2% of stocks over the 1-month, 3-month, and 6-month windows.
- Build the watchlist before the open rather than reacting after the move is obvious.
- After scanning, manually review for a prior real move, higher lows, tightening range, a constructive 2-week to 2-month base, and price riding the 10-day / 20-day area.
- Reject names that are already extended, breaking down below key short moving averages, or sitting directly in front of earnings.
- Sources: [My 3 Timeless Setups](https://qullamaggie.com/my-3-timeless-setups-that-have-made-me-tens-of-millions/), [FAQ](https://qullamaggie.com/faq/).

**Finviz Free**

- Finviz Free is useful for broad discovery, not for fully automating freshness. It is best used to surface strong movers that are then reviewed manually.
- Free Finviz quotes are delayed by 15 minutes. That is acceptable for watchlist building, not for execution timing.
- Finviz does not expose a direct 10-day moving-average screen, so it cannot fully replicate the preferred 10-day / 20-day continuation filter.
- Finviz `Volatility` is only an approximation for ADR because it uses average daily high-low range percentage rather than the exact Qullamaggie ADR formula.
- Official filter reference: [Finviz Screener Help](https://finviz.com/help/screener.ashx).

**Finviz Free community recipe**

- A widely shared Q community recipe comes from Jeff Sun. His goal is a "precise strong mover" scan that normally returns fewer than roughly 60 names, then a global watchlist is maintained and stalked for the right chart shape.
- Baseline filters used across the family:
- Market cap over $300M
- Average volume over 300,000 shares
- Current volume over 100,000 shares
- 1-week mover scan: performance over 1 week greater than 20% plus weekly volatility above 4%.
  Link: [Finviz 1-week strong mover](https://finviz.com/screener.ashx?v=111&f=cap_smallover,sh_avgvol_o300,sh_curvol_o100,ta_perf_1w20o,ta_volatility_wo4&ft=4&o=-marketcap)
- 1-month mover scan: performance over 1 month greater than 30% plus monthly volatility above 5%.
  Link: [Finviz 1-month strong mover](https://finviz.com/screener.ashx?v=111&f=cap_smallover,sh_avgvol_o300,sh_curvol_o100,ta_perf_4w30o,ta_volatility_mo5&ft=4&o=-marketcap)
- 1-month strong-tape variant: performance over 1 month greater than 50%.
  Link: [Finviz 1-month very strong tape](https://finviz.com/screener.ashx?v=111&f=cap_smallover,sh_avgvol_o300,sh_curvol_o100,ta_perf_4w50o,ta_volatility_mo5&ft=4&o=-marketcap)
- 3-month mover scan: performance over 3 months greater than 50%.
  Link: [Finviz 3-month strong mover](https://finviz.com/screener.ashx?v=111&f=cap_smallover,sh_avgvol_o300,sh_curvol_o100,ta_perf_13w50o,ta_volatility_mo5&ft=4&o=-marketcap)
- 6-month mover scan: performance over 6 months greater than 100%.
  Link: [Finviz 6-month strong mover](https://finviz.com/screener.ashx?v=111&f=cap_smallover,sh_avgvol_o300,sh_curvol_o100,ta_perf_26w100o,ta_volatility_mo5&ft=4&o=-marketcap)
- Post-scan workflow in that community recipe:
- Keep one global watchlist rather than expecting the screener to deliver only ready-to-buy names.
- Skip names with earnings inside roughly 5 days.
- Be cautious when the 200-day moving average is still declining overhead.
- Wait for the moving averages to catch up if the stock is still too extended.
- Source: [Jeff Sun's method and flow](https://qullamaggie.net/jeff-suns-method-and-flow/).

**Common Finviz community add-ons**

- Price over $5
- Country set to USA
- Average volume raised to 1,000,000 shares for cleaner execution
- Price above the 200-day SMA
- Monthly volatility above roughly 6% as a rough ADR proxy
- Optional fundamental quality filters such as positive long-run EPS growth
- These are community heuristics, not core Qullamaggie rules.

**TradingView**

- TradingView's stock screener is a stronger free alternative when you want more flexible combinations of performance, volume, relative-volume, and moving-average filters.
- A practical free workflow is to recreate the same 1-month, 3-month, and 6-month strong-mover family there, then manually inspect the charts for freshness.
- Jeff Sun explicitly recommends running both Finviz and TradingView because the symbol overlap is not perfect.
- Qullamaggie's FAQ also links an ADR script for TradingView, which makes TradingView more useful than Finviz when ADR context matters.
- Sources: [TradingView Stock Screener](https://www.tradingview.com/support/solutions/43000718866-tradingview-stock-screener-trade-smarter-not-harder/), [TradingView ADR script linked from Qullamaggie FAQ](https://uk.tradingview.com/script/6KVjtmOY-ADR-Average-Daily-Range-by-MikeC-AKA-TheScrutiniser/).

**base.report**

- `base.report` is one of the better free scanners for Qullamaggie-style continuation work because it exposes ADR%, 20-day volume, 20-day dollar volume, 50-day SMA, 200-day SMA, and range columns directly.
- That makes it better suited than Finviz for filtering out low-ADR, low-dollar-volume, and structurally weak names before chart review.
- One community starter recipe is ADR above roughly 5.5% and 20-day dollar volume above roughly $20M, then manual chart review for a fresh base near the pivot.
- Treat those exact cutoffs as a heuristic starter, not as doctrine.
- Source: [base.report screener](https://base.report/screener).

## Pattern Criteria

**VCP (Volatility Contraction Pattern)**

- At least 2 contractions in price range
- Each contraction tighter than the prior one
- Duration of 2-8 weeks
- Volume declines during the base
- Current week volume at least 50% lower than the prior week

**Flag**

- Prior impulse move at least 20% in 1-4 weeks
- Pullback retraces less than 50% of that impulse
- Consolidation volume declines
- Duration of 5-25 days

**Common criteria**

- Current consolidation range under 15%
- Stock is effectively riding the 10-day and 20-day moving averages with higher lows

> "I find it hard to buy pullbacks, because the best breakouts don't pullback."
> "I buy breakouts as they break out on the daily chart - I don't anticipate breakouts."

**Avoid**

- Names already extended by more than roughly 1 ATR / ADR from the base on the entry day

## What A Good Setup Looks Like

- The stock already had a real prior move, not just one random green candle
- The base is tight and gets tighter
- Volume contracts as the base develops
- Higher lows are visible through the consolidation
- The stock rides the rising 10-day / 20-day area rather than losing it
- The breakout occurs while the stock is still near the pivot, not after a long chase
- The stock is a true leader in a strong or improving market environment

## What A Weak Setup Looks Like

- Only one contraction, with no clear tightening sequence
- Wide, sloppy price swings inside the base
- Deep pullbacks that keep threatening the structure
- Repeated heavy-volume sell candles during the consolidation
- A stock that is “near highs” only because it bounced off a damaged chart
- An average stock in an average or weak group

## Trap Setups And Red Herrings

- **Loose breakout trap:** price pokes through resistance from a messy, high-volatility base and then fails
- **Late-stage extension trap:** the stock already moved too far from the pivot and the trader chases the breakout
- **Anticipation trap:** buying inside the base before the breakout actually confirms
- **News-without-structure trap:** catalyst exists, but the chart never formed a tight continuation base
- **Overhead-supply trap:** stock looks close to a breakout but still has obvious nearby resistance from prior damage

## Entry Checklist

All of the following should be true before entry:

1. Relative strength is in the top tier.
2. The stock is near highs, not far below prior resistance.
3. The base is tight, orderly, and volume contracts during consolidation.
4. Breakout volume is at least 1.5x the 20-day average.
5. The stock is not already too extended from the breakout pivot.
6. The broad tape is not materially fighting long breakouts.

## Entry Timing

| Time                          | Entry                                 |
| ----------------------------- | ------------------------------------- |
| Gap up at open                | Buy 1-minute Opening Range High (ORH) |
| Breaks within first 5 minutes | Buy 5-minute ORH                      |
| Breaks 9:35-10:00am ET        | Buy hourly ORH                        |
| After 10:00am ET              | Enter on daily-chart breakout         |

- If the first 1-minute entry is missed, can add on 5-minute or 60-minute highs
- Buys full size immediately rather than scaling in slowly
- Breakout volume should be greater than 1.5x the 20-day average

> "I buy my full position at once, very aggressively. I just want to get in and get out in a few seconds."

## Stop Loss

- Initial stop is the low of day on the entry day
- Stop should not be wider than 1x ADR
- In exceptional cases can stretch to 1.5x ADR

**Sizing rule**

- Size the trade from the distance between entry and the low-of-day stop using the shared account-risk formula
- If the correct size becomes too small or the stop too wide, skip the trade rather than forcing it

## Partial Profits

- Sell 20-25% of the position, or roughly one-third to one-half, after 3-5 days if the trade is profitable

> "The first move is the most predictable part of the move."

- After the first partial, move the stop on the remainder to break-even

## Trailing Stop

- Trail the remainder with the 10-day MA for faster names or the 20-day MA for slower names
- He currently tends to favor the 20-day MA
- Exit only on a close below the chosen moving average
- Intraday dips below the MA are acceptable
- Maintain a hard stop for catastrophic overnight gaps

## Adding To Winners

- Each add is treated like an independent trade with its own stop
- He will rebuy at higher prices if the stock forms a fresh valid base after more consolidation
- Example cited from CVNA used distinct stops for each add-on tranche

## No-Trade Conditions

- The pattern is loose, obvious distribution is present, or the base is wider than roughly 15%
- Pullback volume expands instead of contracting
- The stock is far from the 10-day / 20-day area and already extended
- No fresh base has formed after the prior breakout; price is simply re-testing highs from an extended move
- The daily chart looks tight but the weekly chart is already visibly late-stage or extended
- The breakout occurs in a weak market regime without exceptional context
- The stock is directly in front of earnings without a profit cushion

## Disciplined Execution Sequence

1. Mark the base pivot and the low of day.
2. Wait for the time-appropriate breakout trigger.
3. Confirm volume and extension are acceptable.
4. Enter full size only if the stop and risk budget still make sense.
5. Do nothing if the breakout does not confirm.
6. Take the first partial only if profitable after 3-5 days.
7. Move the stop to breakeven after the first partial.
8. Let the remainder live or die by the moving-average trailing rule.
