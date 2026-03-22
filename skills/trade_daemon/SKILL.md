---
name: trade-daemon
description: >
  Use for the canonical single-writer trade daemon in the trading-tools
  workspace. Trigger when the task is about observation-source wiring,
  one-shot daemon runs, case seeding/reduction, timer or inbox processing,
  live Kraken BTC wake generation, or the end-to-end daemon half of the wake
  pipeline.
metadata: { "openclaw": { "emoji": "⚙️", "requires": { "bins": ["python3"] } } }
---

# trade-daemon

Use the daemon from `/Users/ad/work/trading-tools/apps/trade_daemon`.

Primary references:

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

- `btc_threshold`
- `crypto_momentum_scalp`
- `gap_watch`
- `rth_breakout`

Look in `/Users/ad/work/trading-tools/apps/trade_daemon/src/trade_daemon/__main__.py` first when you need the live registry or source wiring.

## Source Wiring

Current env-driven observation sources:

- Kraken ticker / OHLC
- Kraken momentum scanner
- Yahoo quotes
- SEC latest filings
- FDA events
- Finnhub news
- TWS snapshots

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
casectl list-open
casectl show-case <case-id>
casectl show-events <case-id>
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
   `btc_threshold` for simple threshold crossings, `crypto_momentum_scalp` for scanner-driven scalp setups
2. inspect or upsert the relevant trigger row with `triggerctl`
3. make sure the daemon has the matching source enabled:
   `TRADE_DAEMON_KRAKEN_PAIRS` for ticker/OHLC paths, `TRADE_DAEMON_KRAKEN_SCANNER_PAIRS` for scanner paths
4. run a one-shot or full daemon loop
5. inspect the resulting case with `casectl`
6. let `assistant-bot` deliver the wake into Discord

The threshold-style BTC path is:

- Kraken ticker observation
- `btc_threshold` strategy
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
- scanner construction is daemon/env-driven, not created dynamically by `triggerctl`
- a new trigger row does not by itself create a new observation source or detector instance

## Known Constraints

- one-shot mode is the safest default for smoke work
- source loops are sequential by design right now
- current case creation is strategy-driven; no generic "create case" CLI exists
- if no trigger row exists, the daemon may ingest observations without ever producing a wake
- `get_effective_trigger(...)` resolves by `strategy_key` plus scope, not by `trigger_key` as a runtime selector
