# Strategy Comparison: Qullamaggie vs. Cameron

Two documented, profitable methodologies. Different timeframes, different mechanics, different edge sources.

---

## At a Glance

| Dimension | Qullamaggie (Swing) | Cameron (Intraday) |
|---|---|---|
| Timeframe | Days to months | Minutes to hours |
| Holding period | 3 days – 3 months | Same day only |
| Setup frequency | 2-5 trades/day scanned; 15-25 held simultaneously | 1-5 trades per morning |
| Win rate | 25-35% | 70-71% |
| Avg win vs. loss | Large wins, small losses (3:1+ R:R) | Small wins, smaller losses (2:1 R:R) |
| Profit model | Few massive winners cover all small losses | High-frequency small gains; consistency |
| Universe | All US stocks >$5, >300K avg volume | Low-float <10M shares, $1-20, gap >4% |
| Market sensitivity | High — needs bull market for breakouts | Lower — best stocks spike regardless of market |
| Account size | Scales with portfolio (any size, but edge erodes >$10M) | Small/medium accounts (large size hits float limits) |
| PDT impact | Swing hold avoids PDT (hold >1 day) | Full intraday day trading; needs $25K or PDT workaround |

---

## Edge Source

**Qullamaggie's edge:**
- Catches multi-week to multi-month trends in relative strength leaders
- Tight entries near consolidation lows → asymmetric risk/reward (small stop, large potential move)
- Low win rate is intentional; one 200% winner covers 10 stop-outs at 1% risk each
- Works because institutional capital chases the same RS leaders, amplifying the move after breakout

**Cameron's edge:**
- Exploits float scarcity: a 5M-share float stock trading 20M shares creates violent supply/demand imbalance
- Enters in the first 5-10 minutes when institutional latency is highest (compliance/committee delays 24-72 hrs)
- High win rate is achievable because: low-float + catalyst + 5× volume = high-probability directional move in the morning window
- Edge degrades after 11:30am as volume dries up and the setup becomes choppier

---

## Position Sizing Comparison

**Qullamaggie:**
```
shares = (equity × risk_pct) / (entry - stop)
risk_pct = 0.0025 to 0.01  (0.25% to 1% of account)
position_size = 5-25% of account
max_single_overnight = 30% of account
```

**Cameron:**
```
shares = acceptable_dollar_loss / stop_distance_per_share
(no formal % of account formula — anchors on fixed share sizes: 3,000 or 6,000 shares)
max_daily_loss = $2,000 per account (hard circuit breaker)
min_r_r = 2:1 required before entering any trade
```

---

## Stop Loss Comparison

**Qullamaggie:** Low of entry day. Maximum width = 1× ATR/ADR (20-day). Never wider than ADR.
Market stops only — executes immediately, no limit.

**Cameron:** Bottom of the consolidation (bull flag low) or opening range low (Gap and Go).
Mental stops; exits if down 20 cents regardless of where "the stop" technically is.

---

## Market Regime

**Qullamaggie:** Requires QQQ's 10 EMA > 20 EMA for taking longs. In downtrends: parabolic shorts only or flat.

**Cameron:** Less regime-dependent. Low-float catalyst stocks can spike in any market. Adjusts by reducing size and selectivity in bear conditions, but does not fully stop.

---

## Scalability

**Qullamaggie:** Erodes above ~$50M-$100M AUM because he can no longer enter meaningfully in lower-liquidity names. He's acknowledged this is now a constraint. His edge was sharpest from $9K to ~$10M.

**Cameron:** Erodes above ~$500K-$2M per trade because float constraints limit share size. With 5M-share float and 1% of ADV rule, maximum meaningful position in a $5 stock trading 10M shares/day = 100,000 shares = $500,000. This is actually an advantage for small accounts.

---

## Which to Use When

| Situation | Recommendation |
|---|---|
| Account <$25K, can't day trade PDT | Qullamaggie swing (hold >1 day, avoids PDT entirely) |
| Account >$25K or has PDT workaround | Either; Cameron for morning action, Qullamaggie for position building |
| Bull market, strong RS leaders | Qullamaggie breakout + EP |
| Any market, strong catalyst stock | Cameron Gap and Go / bull flag |
| Bear market | Cameron (low-float rockets exist in bear markets); Qullamaggie parabolic short only |
| Small account (<$5K) | Cameron's setups have the best edge-per-dollar at small size (float scarcity amplifies moves; low entry price) |
| Want fewer, bigger trades | Qullamaggie |
| Want faster feedback loop | Cameron (daily results, clear right/wrong) |

---

## What Both Agree On

1. **Never trade without a defined stop.** Both use pre-defined exits before entry.
2. **Volume confirmation is mandatory.** Both require significantly above-average volume to validate a setup.
3. **News catalyst matters (especially for gaps/EPs).** Moves without catalyst have lower probability.
4. **Most of the day's opportunity is in the first 1-2 hours.** Cameron stops at 11:30am; Qullamaggie's best setups trigger early in the day.
5. **Cut losses fast, let winners run.** Both describe this as the core behavioral discipline.
6. **Market conditions define your activity level.** Neither trades the same size in all conditions.
