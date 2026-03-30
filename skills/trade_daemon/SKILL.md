---
name: trade-daemon
description: >
  Use for the canonical single-writer trade daemon in the trading-tools
  workspace. Trigger when the task is about observation-source wiring,
  one-shot daemon runs, case seeding/reduction, timer or inbox processing,
  live Kraken BTC wake generation, provider-neutral watch-and-wake setup
  generation from shared completed-bar planning (TWS by default, Kraken for
  explicit crypto pairs), or the end-to-end daemon half of the wake pipeline.
metadata: { "openclaw": { "emoji": "⚙️", "requires": { "bins": ["python3"] } } }
---

# trade-daemon

Use the daemon from `/Users/ad/work/trading-tools/apps/trade_daemon`.

Primary references:

- `/Users/ad/work/trading-tools/docs/architecture/reference_intraday_watch_wake_runtime.md`
- `/Users/ad/work/trading-tools/apps/trade_daemon/src/trade_daemon/__main__.py`
- `/Users/ad/work/trading-tools/apps/trade_daemon/src/trade_daemon/daemon/app.py`
- `/Users/ad/work/trading-tools/apps/trade_daemon/src/trade_daemon/infrastructure/fixture_loader.py`
- `/Users/ad/work/trading-tools/scripts/smoke/smoke_daemon_workflow.sh`
- `/Users/ad/work/trading-tools/scripts/smoke/smoke_edge_alert_flow.sh`
- `/Users/ad/work/trading-tools/scripts/smoke/fixtures/gap_watch_seed.json`
- `/Users/ad/work/trading-tools/scripts/smoke/fixtures/gap_watch_break.json`
- `/Users/ad/work/trading-tools/scripts/smoke/fixtures/review_decision_approve.json`

## What It Owns

`trade-daemon` is the canonical single writer for:

- source observation ingestion
- trigger lookup
- trigger fanout for same-symbol watch families
- strategy seed/reduce decisions
- canonical case/event/timer/wake/review writes
- daemon inbox processing
- timer processing

It is not:

- a Discord bot
- a human operator UI
- a journaling CLI

## When To Use

- "run the daemon once"
- "seed a case from a fixture"
- "consume a review decision from the inbox"
- "process timers once"
- "wire a live source"
- "why did this wake fire?"
- "test the Kraken BTC feed"
- "verify the daemon half of the alert pipeline"

## Stable Command Surface

Use:

- `trade-daemon`
- `python -m trade_daemon --help`

Current subcommands:

- `run`
- `oneshot`

## Strategy Registry

The current built-in strategies are:

- `threshold_watch`
- `crypto_momentum_scalp`
- `level_watch`
- `vwap_bounce_watch`
- `vwap_reclaim_watch`
- `gap_watch`
- `rth_breakout`

Look in `/Users/ad/work/trading-tools/apps/trade_daemon/src/trade_daemon/__main__.py` first when you need the live registry or source wiring.

## Source Wiring

Current source families combine static env inputs with trigger-driven planning:

- Kraken ticker / OHLC
- Kraken momentum scanner (trigger-driven for `crypto_momentum_scalp` when a trigger store is present; env pairs remain bootstrap/fallback inputs)
- Yahoo quotes
- SEC latest filings
- FDA events
- Finnhub news
- TWS snapshots
- shared completed-bar planning for watch families:
  - TWS is the default stock / explicit contract path
  - Kraken is supported for explicit crypto pair triggers

Useful env examples:

```bash
TRADE_DAEMON_KRAKEN_PAIRS=BTC/USD
TRADE_DAEMON_KRAKEN_EVENT_TRIGGER=trades
TRADE_DAEMON_KRAKEN_OHLC_ENABLED=1
TRADE_DAEMON_KRAKEN_INTERVAL_MINUTES=1
```

```bash
TRADE_DAEMON_KRAKEN_SCANNER_PAIRS=BTC/USD
TRADE_DAEMON_KRAKEN_SCANNER_EVENT_TRIGGER=bbo
TRADE_DAEMON_KRAKEN_SCANNER_EXECUTION_BAR_SECONDS=300
TRADE_DAEMON_KRAKEN_SCANNER_MIN_TOTAL_SCORE=54
TRADE_DAEMON_KRAKEN_SCANNER_MAX_PULLBACK_PCT=0.45
TRADE_DAEMON_KRAKEN_SCANNER_MAX_PULLBACK_BARS=8
```

```bash
TRADE_DAEMON_YAHOO_SYMBOLS=AAPL,NVDA
```

```bash
TRADE_DAEMON_SEC_USER_AGENT="Your Name your_email@example.com"
TRADE_DAEMON_SEC_FORMS=8-K,6-K
```

```bash
TRADE_DAEMON_TWS_INTRADAY_MODE=poll
TRADE_DAEMON_TWS_INTRADAY_POLL_INTERVAL_SECONDS=15
TRADE_DAEMON_TWS_INTRADAY_FINALIZE_GRACE_SECONDS=15
```

Important:

- the canonical watch families consume `market.bar`
- completed-bar planning is shared/provider-neutral first; provider capability comes second
- the default intraday runtime is `poll`
- `subscribe` exists, but it should be treated as gated by a market-hours cadence canary
- use explicit crypto pair metadata for Kraken watch triggers; there is no silent symbol-to-pair guessing
- use explicit contract metadata (`sec_type`, `exchange`, `currency`, etc.) when the instrument is not a default stock contract

## One-shot Workflow

Prefer one-shot seams before infinite loops.

Fixture source run:

```bash
python -m trade_daemon oneshot \
  --fixture /Users/ad/work/trading-tools/scripts/smoke/fixtures/gap_watch_seed.json \
  --skip-inbox \
  --skip-timers
```

Fixture inbox run:

```bash
python -m trade_daemon oneshot \
  --inbox-fixture /Users/ad/work/trading-tools/scripts/smoke/fixtures/review_decision_approve.json \
  --skip-sources \
  --skip-timers
```

Live Kraken run:

```bash
TRADE_DAEMON_KRAKEN_PAIRS=BTC/USD python -m trade_daemon oneshot --skip-inbox --skip-timers
```

## What To Inspect After A Run

Use `casectl` and SQL to verify:

- `cases.trade_cases`
- `cases.case_events`
- `cases.wake_requests`
- `cases.review_requests`
- `cases.review_decisions`
- `cases.case_timers`
- `cases.daemon_inbox`

Typical operator checks:

```bash
casectl list-open-case-summaries
casectl show-case-summary <case-id>
casectl show-case-event-journal <case-id>
```

## Smoke Scripts

Bootstrap DB and seed trigger defaults:

```bash
/Users/ad/work/trading-tools/scripts/smoke/smoke_db_bootstrap.sh
```

Daemon fixture workflow:

```bash
/Users/ad/work/trading-tools/scripts/smoke/smoke_daemon_workflow.sh
```

Full edge alert path:

```bash
/Users/ad/work/trading-tools/scripts/smoke/smoke_edge_alert_flow.sh
```

Install-level package smoke:

```bash
/Users/ad/work/trading-tools/scripts/smoke/smoke_install_surfaces.sh
```

## BTC / Kraken Guidance

If the task is about live BTC alerts:

1. identify which strategy family owns the setup:
   `threshold_watch` for simple threshold crossings, `crypto_momentum_scalp` for scanner-driven scalp setups
2. inspect or upsert the relevant trigger row with `triggerctl list-trigger-definitions`, `triggerctl show-trigger-definition`, or `triggerctl upsert-trigger-definition`
3. make sure the daemon has the matching source/planning path available:
   `TRADE_DAEMON_KRAKEN_PAIRS` for static quote/OHLC bootstrap, and enabled `crypto_momentum_scalp` triggers for scanner planning when a trigger store is present (`TRADE_DAEMON_KRAKEN_SCANNER_PAIRS` remains the bootstrap/fallback pair list)
4. run a one-shot or full daemon loop
5. inspect the resulting case with `casectl`
6. let `assistant-bot` deliver the wake into Discord

The threshold-style BTC path is:

- canonical `market.quote` observation
- `threshold_watch` strategy
- canonical wake request
- `wake_dispatch`
- `assistant-bot` Discord delivery

The crypto momentum scalp path is:

- Kraken momentum scanner observation source
- `crypto_momentum_scalp` strategy
- canonical wake request
- `wake_dispatch`
- `assistant-bot` Discord delivery

Important runtime detail:

- trigger rows configure reducer behavior and routing after observations exist
- when a trigger store is present, crypto momentum scanner planning is trigger-driven rather than env-only
- `TRADE_DAEMON_KRAKEN_SCANNER_PAIRS` is still a bootstrap/fallback pair list and env bridge, not the canonical control surface for trigger-backed planning
- a new trigger row still depends on the daemon runtime being up and loading the trigger-backed source planner; it does not create an out-of-process detector worker on its own

## Watch Guidance

If the task is about "watch this stock", "watch this crypto pair", "alert at this level", "wake the AI bot on a VWAP bounce", or similar persistent monitoring:

1. identify the watch family:
   `level_watch` for fixed price levels,
   `vwap_bounce_watch` for touch-and-confirm bounce behavior,
   `vwap_reclaim_watch` for reclaim-after-loss behavior
2. create or inspect the trigger row with `triggerctl validate-trigger-parameters`, `triggerctl list-trigger-definitions`, or `triggerctl upsert-trigger-definition`
3. make sure the trigger payload clearly identifies the instrument:
   - for TWS/instrument-contract paths, include explicit instrument metadata (`sec_type`, `exchange`, `currency`, etc.)
   - for Kraken watch bars, include explicit pair metadata (`pair`, `provider_symbol`, or instrument metadata with pair); there is no silent symbol-to-pair guessing
4. run a one-shot or full daemon loop
5. inspect the resulting case and wake via `casectl`
6. let `assistant-bot` deliver the Discord wake

These watch families are alert-only. They do not auto-execute trades and they do not encode the downstream AI decision.

## Trigger Routing Model

There are two main runtime trigger paths:

- effective-trigger lookup
  used by phase-1 / scanner-driven families such as `gap_watch`, `rth_breakout`, `threshold_watch`, and `crypto_momentum_scalp`

- same-symbol watch fanout
  used by `level_watch`, `vwap_bounce_watch`, and `vwap_reclaim_watch`

For the watch families:

- `strategy_key` is the reducer family
- `trigger_key` is the operator-facing label
- `trigger_id` is the runtime identity
- one canonical `market.bar` observation can seed or reduce multiple same-symbol watch cases
- the completed-bar planner is shared/provider-neutral; provider capability is chosen after canonical intent resolution

## Known Constraints

- one-shot mode is the safest default for smoke work
- source loops are sequential by design right now
- current case creation is strategy-driven; no generic "create case" CLI exists
- if no trigger row exists, the daemon may ingest observations without ever producing a wake
- phase-1/scanner families still use effective-trigger lookup by `strategy_key` plus scope rather than `trigger_key` as a runtime selector
- the watch families fan out over every matching same-symbol trigger row from the canonical watch set
