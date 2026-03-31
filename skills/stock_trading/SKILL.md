---
name: stock-trading
description: >
  Umbrella workflow for discretionary and systematic stock-trading support
  using the standalone trading-tools workspace plus local playbook references.
  Use when asked to scan stocks, analyze market context, inspect canonical
  cases, configure persistent triggers, compare intraday vs swing playbooks,
  combine TWS, trade_journal, triggerctl, and macro-dashboard outputs into one
  trading view, render and share trading charts, or reason about the current
  daemon-plus-assistant-bot alert pipeline.
metadata: { "openclaw": { "emoji": "📈", "requires": { "bins": ["uv"] } } }
---

# stock-trading

Use this as the top-level stock trading workflow when the task spans more than one tool or needs a full trading process:

- discovery and live market context
- macro regime context
- canonical case tracking and lifecycle inspection
- persistent alert creation and monitoring
- execution linkage and review
- playbook lookup for deeper setup context

Primary tool docs:

- `/Users/ad/work/trading-tools/README.md`
- `/Users/ad/work/trading-tools/docs/architecture/reference_intraday_watch_wake_runtime.md`
- `/Users/ad/work/trading-tools/packages/trade_tws/src/trade_tws/README.md`
- `/Users/ad/work/trading-tools/packages/trade_journal/src/trade_journal/README.md`
- `/Users/ad/work/trading-tools/packages/trade_yahoo/src/trade_yahoo/README.md`
- `/Users/ad/work/trading-tools/packages/trade_yahoo/src/trade_yahoo/macro_dashboard.py`
- `/Users/ad/work/trading-tools/packages/trade_sec/src/trade_sec/README.md`
- `/Users/ad/work/ai/openclaw/skills/stock_charting/SKILL.md`
- `/Users/ad/work/ai/openclaw/skills/assistant_bot/SKILL.md`
- `/Users/ad/work/ai/openclaw/skills/trade_daemon/SKILL.md`

End-to-end autonomous lifecycle playbooks:

- `/Users/ad/dotfiles/docs/domains/trading/strategies/qullamaggie-ep-autonomous-lifecycle.md`
- `/Users/ad/dotfiles/docs/domains/trading/strategies/qullamaggie-continuation-breakout-autonomous-lifecycle.md`
- `/Users/ad/dotfiles/docs/domains/trading/strategies/ross-cameron-gap-and-go-autonomous-lifecycle.md`
- `/Users/ad/dotfiles/docs/domains/trading/strategies/ross-cameron-bull-flag-breakout-autonomous-lifecycle.md`

Source rules / supporting methodology docs:

- `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-rules.md`
- `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-gap-and-go.md`
- `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-bull-flag.md`
- `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-flat-top-breakout.md`
- `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-abcd.md`
- `/Users/ad/dotfiles/docs/domains/trading/strategies/qullamaggie-continuation-breakout-scored-checklist.md`
  Canonical weighted continuation-breakout checklist. Use this first when scanning candidates, use it again when reviewing charts, and cite it when grading setups or planning trades.
- `/Users/ad/work/ai/openclaw/skills/stock_trading/references/qullamaggie-rules.md`
  OpenClaw root rules reference for the Qullamaggie family, including the full continuation-breakout checklist, hard fails, ADR overlay, and inferred scan overlays.
- `/Users/ad/dotfiles/docs/domains/trading/strategies/qullamaggie-breakout.md`
  Execution and scan heuristics for Qullamaggie breakout work. Use alongside the scored checklist for trigger timing, freshness, and anti-chase rules.
- `/Users/ad/dotfiles/docs/domains/trading/strategies/qullamaggie-rules.md`
  Long-form dotfiles rules mirror. Use this when you need the fuller methodology and provenance behind the checklist and overlays.
- `/Users/ad/work/trading-tools/knowledge_base/10-Entities/Strategies/Qullamaggie.md`
  Strategy-level summary for the broader Qullamaggie approach.
- `/Users/ad/work/trading-tools/knowledge_base/10-Entities/Setups/Qullamaggie Continuation Breakout.md`
  Setup-level continuation-breakout note. Use after the scored checklist to confirm setup identity, workflow fit, and trade-planning language.
- `/Users/ad/work/trading-tools/knowledge_base/10-Entities/Setups/Qullamaggie Episodic Pivot.md`
  Companion setup note for episodic pivots when a candidate is not actually a continuation breakout.
- `/Users/ad/work/ai/openclaw/skills/stock_trading/references/qullamaggie-parabolic-short.md`
  Complementary short-side reference for parabolic exhaustion cases that are the opposite of long continuation breakouts.
- `/Users/ad/dotfiles/docs/domains/trading/strategies/strategy-comparison.md`

## When to use

- "scan for stocks and tell me what matters"
- "build a trade plan"
- "track this setup as a canonical case"
- "how does macro affect this setup?"
- "compare a Qullamaggie swing versus a Ross Cameron intraday trade"
- "show me candidate longs/shorts and record the thesis"
- "inspect the case, trigger, or review state around this setup"
- "plot this and send the chart"
- "watch this stock and alert me later"

## Workflow

1. Establish macro and market regime first.
2. Run discovery / live checks for symbols or sectors.
3. Decide the trade style: intraday momentum, swing breakout, event-driven, or no trade.
4. If the trigger should keep running after the current chat, configure or inspect canonical trigger rows with `triggerctl`.
5. Make sure `trade-daemon` is running with the source that can actually emit the needed observations.
6. Record and inspect the resulting canonical case state with `casectl`.
7. After execution or operator review, follow the case through daemon inbox + canonical events.

## Tool selection

### `tws` for live discovery and broker-facing market data

Use when you need:

- scanners
- quote snapshots
- bars from IBKR
- positions / executions / contracts / options / news

Default entrypoint:

```bash
tws ...
```

Common examples:

```bash
tws scanner run --scan-code TOP_PERC_GAIN
tws market snapshot AAPL --delayed
tws market bars AAPL NVDA --duration "30 D" --bar-size "1 day" --what-to-show TRADES
tws account positions
```

### `macro-dashboard` for cross-asset regime context

Use when you need:

- oil / gold / dollar / VIX context
- yield / credit / EM stress proxies
- correlation, z-score, vol, and geopolitical overlays
- a regime filter before taking equity trades

Default entrypoint:

```bash
yfinance macro-dashboard --json
```

Targeted section:

```bash
yfinance macro-dashboard --section 9 --json
```

### `stock-charting` for rendered charts and Discord-ready images

Use when you need:

- line or candlestick charts
- support / resistance and trend lines
- indicator overlays and lower panels
- equity curves or backtest charts
- a real PNG/JPG uploaded back to chat

Important:

- render locally
- send the file with the message tool
- do not rely on `read` as the attachment mechanism

### `assistant-bot` for wake delivery and Discord operator intake

Use when you need:

- wake alerts delivered into Discord
- the same notification surface that human Discord users see
- review / note / command intake through Discord
- downstream verification that a daemon-created wake actually reached Discord

Important:

- assistant-bot no longer creates or manages persistent alerts
- use assistant-bot through Discord, not by importing its code or inventing a relay API
- use `triggerctl` plus `trade-daemon` to create the condition that will later emit a wake
- then use the `assistant-bot` skill to inspect delivery or shared-channel behavior
- if the wake should explicitly ping another Discord actor, use `discord_mention_text` on the trigger row or the daemon-side default mention env

### `trade-journal` skill for canonical case inspection and trigger control

Use when you need:

- list or inspect live canonical cases
- inspect canonical event history
- inspect or mutate persistent trigger rows
- bootstrap default strategy triggers
- verify the daemon-created case state behind an alert

Default entrypoint:

```bash
casectl ...
```

Common examples:

```bash
casectl list-open-case-summaries
casectl show-case-summary <case-id>
casectl show-case-event-journal <case-id>
triggerctl list-trigger-definitions
triggerctl show-default-trigger-parameters --strategy-key gap_watch
```

Daily quick set (concise):

```bash
# 1) Active canonical cases
casectl list-open-case-summaries

# 2) Inspect one case
casectl show-case-summary <case-id>
casectl show-case-event-journal <case-id>

# 3) Preview or validate trigger params
triggerctl show-default-trigger-parameters --strategy-key gap_watch
triggerctl validate-trigger-parameters --strategy-key gap_watch --parameters-json '{"min_gap_pct":0.04,"min_price":2.0,"min_premarket_volume":500000,"or_minutes":5,"expiry_minutes":90}'

# 4) Upsert or seed persistent trigger rows
triggerctl seed-default-trigger-definitions --strategy-key gap_watch --apply
triggerctl upsert-trigger-definition --strategy-key threshold_watch --trigger-key btc-threshold-main --scope-kind symbol --scope-ref BTCUSD --parameters-json '{...}'
```

For the stock watch families, skip `seed-default-trigger-definitions` and validate explicit JSON instead.

## Trigger model

- `strategy_key` names the reducer family such as `gap_watch`, `threshold_watch`, or `crypto_momentum_scalp`
- `trigger_key` names one deployed configuration of that family
- trigger rows are persistent config, not standalone runtime workers
- `assistant-bot` is delivery-only; it does not create alerts
- if a setup depends on a specific market feed or scanner, `trade-daemon` still needs the matching source/planning path active; for crypto momentum this is trigger-driven when a trigger store is present, with env values retained as bootstrap/fallback inputs

Current generic watch families:

- `threshold_watch`
- `level_watch`
- `vwap_bounce_watch`
- `vwap_reclaim_watch`

Important runtime detail:

- the watch families support same-symbol trigger fanout
- `trigger_id` is the runtime identity for one deployed watch
- one canonical completed bar can feed more than one same-symbol watch case
- provider capability comes after canonical completed-bar intent resolution:
  - TWS is the default stock / explicit contract path
  - Kraken is supported for explicit crypto pair metadata

### `sec` for SEC / EDGAR filing intake and repair

Use when you need:

- recent 8-K / 6-K filings
- SEC filing checks for a symbol / company / CIK
- filing streams and reconcile / repair
- local export of filing state for downstream analysis

Default entrypoint:

```bash
SEC_USER_AGENT="Your Name your_email@example.com" sec ...
```

Common examples:

```bash
SEC_USER_AGENT="Your Name your_email@example.com" sec latest --forms 8-K,6-K --format json
SEC_USER_AGENT="Your Name your_email@example.com" sec latest --forms 8-K --company NVIDIA --format json
SEC_USER_AGENT="Your Name your_email@example.com" sec reconcile --date 2026-03-11 --format json
sec export --format json
```

## How to combine them

### Intraday momentum workflow

1. Use `macro-dashboard` first to decide whether the tape is supportive, mixed, or hostile for momentum.
2. Use `tws scanner` and `market snapshot` / `market bars` to build a watchlist.
3. Compare the candidates to the intraday playbook in:
   - `/Users/ad/dotfiles/docs/domains/trading/strategies/ross-cameron-gap-and-go-autonomous-lifecycle.md`
   - `/Users/ad/dotfiles/docs/domains/trading/strategies/ross-cameron-bull-flag-breakout-autonomous-lifecycle.md`
   - `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-rules.md`
   - `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-gap-and-go.md`
   - `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-bull-flag.md`
   - `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-flat-top-breakout.md`
   - `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-abcd.md`
4. Record the thesis and trigger as a canonical strategy/trigger row combination.
5. If the trigger should be watched after the current session, upsert the canonical trigger row and make sure the daemon source is running.
6. Inspect the resulting case and wake flow with `casectl` and `assistant-bot`.

For persistent alerts that should wake the AI later, prefer the canonical watch families:

- `threshold_watch` for quote-driven threshold alerts
- `level_watch` for completed-bar fixed levels
- `vwap_bounce_watch`
- `vwap_reclaim_watch`

Do not default to inventing a full autonomous Ross bull-flag detector when the actual requirement is a reliable wake for later judgment.

### Swing breakout workflow

1. Use `macro-dashboard` to decide whether the broader regime supports breakout continuation.
2. Use `tws market bars` or existing catalog/backtest data for the name.
3. Start with the scored continuation-breakout checklist, then compare the setup to:
   - `/Users/ad/dotfiles/docs/domains/trading/strategies/qullamaggie-continuation-breakout-scored-checklist.md`
   - `/Users/ad/dotfiles/docs/domains/trading/strategies/qullamaggie-continuation-breakout-autonomous-lifecycle.md`
   - `/Users/ad/dotfiles/docs/domains/trading/strategies/qullamaggie-breakout.md`
   - `/Users/ad/dotfiles/docs/domains/trading/strategies/qullamaggie-ep-autonomous-lifecycle.md`
   - `/Users/ad/work/ai/openclaw/skills/stock_trading/references/qullamaggie-rules.md`
   - `/Users/ad/dotfiles/docs/domains/trading/strategies/qullamaggie-rules.md`
   - `/Users/ad/work/trading-tools/knowledge_base/10-Entities/Strategies/Qullamaggie.md`
   - `/Users/ad/work/trading-tools/knowledge_base/10-Entities/Setups/Qullamaggie Continuation Breakout.md`
   - `/Users/ad/work/trading-tools/knowledge_base/10-Entities/Setups/Qullamaggie Episodic Pivot.md`
   - `/Users/ad/work/ai/openclaw/skills/stock_trading/references/qullamaggie-parabolic-short.md`
   - `/Users/ad/dotfiles/docs/domains/trading/strategies/strategy-comparison.md`
     Treat the checklist as mandatory in three phases:
   - scanning: reject names that fail the hard-fail items or clearly miss the weighted threshold
   - setup review: score every serious candidate criterion by criterion, including unchecked items
   - trade planning: use the checklist result to justify buy zone, invalidation, extension risk, and whether the name is actionable now or needs a reset
4. Inspect or seed the relevant canonical trigger rows before execution.
5. If the trade depends on a future breakout or invalidation level, let `trade-daemon` monitor it and let `assistant-bot` deliver the wake.
6. Review outcome quality later through canonical case history and inbox-driven review events.
   Re-open the scored checklist during review so setup quality, execution quality, and freshness can be judged separately.

### Event / catalyst workflow

1. Use `tws news` and scanner output to identify the catalyst name.
2. Use `sec latest` or `sec reconcile` when the catalyst may be tied to a fresh filing.
3. Use `macro-dashboard` to determine whether the catalyst is fighting or aligned with the broader tape.
4. Use the canonical trigger + case path to record:
   - strategy
   - catalyst
   - invalidation
   - wake/review progression
5. If the setup needs waiting or follow-through monitoring, upsert the trigger row and verify daemon coverage for the relevant source.
6. If needed, compare the setup to intraday or swing playbooks before choosing the holding period.

## Universal guardrails

Apply these to any strategy or playbook unless the user explicitly overrides them.

- Enter on confirmation, not anticipation.
  Do not buy blind catches, early knife-catches, or unconfirmed breakouts just because price is lower or moving fast.
- Define invalidation before entry.
  Every trade should have a clear "I am wrong" line plus hard-stop and soft-stop behavior.
- Use fixed-fraction risk and stable sizing.
  Risk per trade should be pre-sized from stop distance; do not increase size because emotions rise or the trade is under water.
- Never average down into a loser.
  Adds are only for improved confirmation or a fresh trigger, not for loss repair.
- Exit failed momentum quickly.
  If the expected extension does not happen, or reclaim/breakout structure fails, reduce or exit without debate.
- Re-enter only on a new setup.
  A stopped-out trade may be re-entered only on a fresh trigger with a fresh plan, not by repairing the prior mistake.
- Respect daily loss and cool-off controls.
  Use max daily loss, max consecutive-loss, and post-stop cooldown rules to prevent tilt and size-creep behavior.
- Check regime and data quality first.
  Reduce size or do not trade when the macro tape, liquidity, spread/volume quality, or catalyst quality is hostile or unreliable.
- Log thesis, trigger, invalidation, and risk in the canonical case/trigger system.
  Treat case inspection and trigger review as part of trade approval, not post-hoc cleanup.
- Separate execution quality from thesis quality.
  A good thesis with bad execution is still a bad trade; record both explicitly in reviews.

## Decision heuristics

- Use intraday momentum playbooks when:
  - the move is catalyst-driven now
  - liquidity and volume are concentrated in the open
  - the edge depends on speed, HOD, VWAP, and opening-range behavior

- Use swing playbooks when:
  - the move is building over days to weeks
  - market regime and trend alignment matter more than minute-to-minute tape
  - the setup resembles EP / breakout / parabolic structures rather than a morning scalp

- Use macro data mostly as:
  - a filter
  - a sizing input
  - a hold-time input
    not as the exact entry trigger

## Equity curve benchmarking (0% to 5% daily bands)

Use this when the user wants to compare live account progress against fixed daily compounding scenarios (for example 0%, 1%, 2%, 3%, 4%, 5%).

1. Pull current account value from TWS (prefer NetLiquidation):

```bash
tws --format json account overview
```

2. Render multi-line projection chart with log Y-axis using Python tooling.

Reference implementation pattern:

- pull the latest net liquidation value from `tws account overview`
- render the comparison chart locally with the charting workflow in `/Users/ad/work/ai/openclaw/skills/stock_charting/SKILL.md`
- keep all daily-rate scenarios on one log-scale figure

3. Share the chart back to chat as a real media attachment/path (not plain text).

Notes:

- Keep all rates on the same figure.
- Prefer log Y-axis so early and late periods are both legible.
- Include title, axis labels, legend, and grid.
- If requested, overlay an observed live equity series on top of the scenario bands.

## Canonical Alert Pattern

Use the daemon + trigger store when the user wants ongoing monitoring, not just one-off analysis.

Recommended sequence:

1. analyze the symbol first with `tws`, `macro-dashboard`, `sec`, or charting as needed
2. identify the exact trigger or invalidation level
3. if the condition should persist beyond the current chat, upsert a canonical trigger row with `triggerctl`
4. make sure `trade-daemon` is running with the relevant source
5. if the thesis matters, inspect the resulting case state with `casectl`

Examples:

- swing breakout above a defined pivot:
  - use the strategy-specific trigger params rather than a generic `price_above` envelope
- BTC threshold on Kraken:
  - use `threshold_watch` plus `triggerctl upsert-trigger-definition`
- BTC momentum scalp on Kraken:
  - use `crypto_momentum_scalp` plus `triggerctl upsert-trigger-definition`
  - prefer explicit instrument/pair metadata in the trigger; when a trigger store is present, the scanner path is planned from triggers and `TRADE_DAEMON_KRAKEN_SCANNER_PAIRS` is just a bootstrap/fallback bridge
- stock or crypto key level:
  - use `level_watch` plus `triggerctl upsert-trigger-definition`
  - for TWS/instrument-contract paths, include explicit contract metadata for crypto (`sec_type`, `exchange`, `currency`) when needed
  - for Kraken watch bars, include explicit pair metadata; there is no silent symbol-to-pair guessing
- stock or crypto VWAP bounce or reclaim:
  - use `vwap_bounce_watch` or `vwap_reclaim_watch`
  - for TWS/instrument-contract paths, include explicit contract metadata when needed
  - for Kraken watch bars, include explicit pair metadata when needed
  - use `discord_mention_text` if the Discord wake should explicitly ping an AI bot or role
- follow-up review handling:
  - inspect case state with `casectl show-case-summary` and `casectl show-case-event-journal`

OpenClaw delivery guidance:

- assistant-bot is downstream only
- OpenClaw should watch the shared Discord channel for delivered wakes
- use the `assistant-bot` skill for delivery-side debugging
- do not claim a persistent alert exists until the trigger row is present and daemon ingestion is running

## Defaults

- Prefer `--json` whenever outputs will be summarized or chained into another tool step.
- Do not place or modify broker orders unless the user explicitly asks.
- When the task includes "watch this", "alert me", "notify later", or any persistent trigger, prefer `triggerctl` + `trade-daemon` over ad hoc reminders.
- Use `sec` when a thesis depends on a fresh filing rather than only headlines or broker news.
- For live market data, prefer read-only and delayed-safe queries first.
- If the task is narrow and only one tool is needed, use that narrower tool directly instead of forcing the whole workflow.
