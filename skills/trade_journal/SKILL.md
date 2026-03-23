---
name: trade-journal
description: >
  Canonical case-engine inspection and trigger tooling for the standalone
  trading-tools workspace. Use when asked about active trade cases,
  wake/review/event inspection, trigger configuration, daemon-created setups,
  or local smoke verification of the current case engine.
metadata: { "openclaw": { "emoji": "🗃️", "requires": { "bins": ["uv"] } } }
---

# trade-journal

Use this skill for the canonical case-engine tooling built from:

- `trade_journal`
- `trade_triggers`
- `trade-daemon`

Primary docs:

- `/Users/ad/work/trading-tools/docs/architecture/reference_intraday_watch_wake_runtime.md`
- `/Users/ad/work/trading-tools/packages/trade_journal/README.md`
- `/Users/ad/work/trading-tools/packages/trade_triggers/README.md`
- `/Users/ad/work/trading-tools/apps/trade_daemon/src/trade_daemon/__main__.py`
- `/Users/ad/work/trading-tools/scripts/smoke/smoke_db_bootstrap.sh`
- `/Users/ad/work/trading-tools/scripts/smoke/smoke_daemon_workflow.sh`

## When to use

- "show my active cases"
- "what fired this wake alert?"
- "show me the event history for this setup"
- "which trigger is active for this strategy or symbol?"
- "which same-symbol watch triggers are active for this family?"
- "seed default triggers"
- "inspect the canonical daemon state for this case"
- "confirm the current case engine is wired and bootstrapped"

## Current Tools

Case inspection:

- `casectl list-open-case-summaries`
- `casectl show-case-summary <case-id>`
- `casectl show-case-event-journal <case-id>`

Trigger inspection and mutation:

- `triggerctl show-default-trigger-parameters --strategy-key <strategy-key>`
- `triggerctl validate-trigger-parameters --strategy-key <strategy-key> --parameters-json '<json>'`
- `triggerctl list-trigger-definitions`
- `triggerctl show-trigger-definition --trigger-id <trigger-id>`
- `triggerctl upsert-trigger-definition ...`
- `triggerctl disable-trigger-definition --trigger-id <trigger-id>`
- `triggerctl seed-default-trigger-definitions --strategy-key <strategy-key> [--apply]`

## Defaults

- Prefer the installed shims:
  - `casectl ...`
  - `triggerctl ...`
- Fallback module entrypoints:
  - `python -m trade_journal --help`
  - `python -m trade_triggers --help`
- Prefer JSON output whenever the result will be summarized or chained
- Treat `casectl` as read-only inspection
- Treat `triggerctl` as the operator surface for persistent trigger configuration

## Quick start

List active cases:

```bash
casectl list-open-case-summaries
```

Inspect one canonical case:

```bash
casectl show-case-summary <case-id>
casectl show-case-event-journal <case-id>
```

Preview strategy defaults:

```bash
triggerctl show-default-trigger-parameters --strategy-key gap_watch
triggerctl show-default-trigger-parameters --strategy-key rth_breakout
triggerctl show-default-trigger-parameters --strategy-key btc_threshold
triggerctl show-default-trigger-parameters --strategy-key crypto_momentum_scalp
```

Important:

- `show-default-trigger-parameters` and `seed-default-trigger-definitions` are still most useful for the older phase-1 families above
- the newer stock watch families are usually hand-validated and hand-upserted today

Validate or seed defaults:

```bash
triggerctl validate-trigger-parameters --strategy-key gap_watch --parameters-json '{"min_gap_pct":0.04,"min_price":2.0,"min_premarket_volume":500000,"or_minutes":5,"expiry_minutes":90}'
triggerctl seed-default-trigger-definitions --strategy-key gap_watch --apply
triggerctl seed-default-trigger-definitions --strategy-key rth_breakout --apply
triggerctl seed-default-trigger-definitions --strategy-key crypto_momentum_scalp --apply
triggerctl list-trigger-definitions
```

Create or update one symbol-scoped trigger:

```bash
triggerctl upsert-trigger-definition \
  --strategy-key btc_threshold \
  --trigger-key btc-threshold-kraken \
  --scope-kind symbol \
  --scope-ref BTCUSD \
  --parameters-json '{"pair":"BTC/USD","venue":"KRAKEN","threshold_price":90000,"direction":"above","cooldown_minutes":15}'
```

Create or update one crypto momentum scalp trigger:

```bash
triggerctl upsert-trigger-definition \
  --strategy-key crypto_momentum_scalp \
  --trigger-key btcusd-long \
  --scope-kind symbol \
  --scope-ref BTCUSD \
  --parameters-json '{"venue":"KRAKEN","symbol":"BTCUSD","direction":"long","session_grade_min":"C","min_liquidity_score":18,"min_urgency_score":20,"min_friction_score":12,"min_total_score":54,"min_threshold_bps":8,"cusum_sigma_multiplier":2.5,"max_pullback_pct":0.45,"max_pullback_bars":8,"setup_ttl_seconds":90,"entry_live_ttl_seconds":30,"notify_invalidations":true,"discord_channel_id":"1234567890"}'
```

Create or update one stock level watch trigger:

```bash
triggerctl validate-trigger-parameters \
  --strategy-key equity_level_watch \
  --parameters-json '{"venue":"SMART","symbol":"AAPL","timeframe":"1m","level_price":250,"direction":"above","cooldown_minutes":30,"discord_channel_id":"1234567890","discord_mention_text":"<@1234567890>"}'

triggerctl upsert-trigger-definition \
  --strategy-key equity_level_watch \
  --trigger-key aapl-250-break \
  --scope-kind symbol \
  --scope-ref AAPL \
  --parameters-json '{"venue":"SMART","symbol":"AAPL","timeframe":"1m","level_price":250,"direction":"above","cooldown_minutes":30,"discord_channel_id":"1234567890","discord_mention_text":"<@1234567890>"}'
```

Create or update one stock VWAP bounce trigger:

```bash
triggerctl validate-trigger-parameters \
  --strategy-key equity_vwap_bounce_watch \
  --parameters-json '{"venue":"SMART","symbol":"AAPL","timeframe":"1m","reference_sigma":0,"touch_tolerance_bps":5,"max_overshoot_bps":20,"alert_ttl_minutes":20,"notify_invalidations":true,"discord_channel_id":"1234567890"}'

triggerctl upsert-trigger-definition \
  --strategy-key equity_vwap_bounce_watch \
  --trigger-key aapl-vwap-bounce \
  --scope-kind symbol \
  --scope-ref AAPL \
  --parameters-json '{"venue":"SMART","symbol":"AAPL","timeframe":"1m","reference_sigma":0,"touch_tolerance_bps":5,"max_overshoot_bps":20,"alert_ttl_minutes":20,"notify_invalidations":true,"discord_channel_id":"1234567890"}'
```

## How to think about the current system

- cases are created by `trade-daemon` reducers, not by a journal CLI create command
- triggers live in Postgres and are read by the daemon through `TimescaleTriggerStore`
- wake/review/timer artifacts are canonical rows, not ad hoc alert-rule records
- `assistant-bot` only delivers wakes and writes operator intake back to `cases.daemon_inbox`
- `strategy_key` identifies the reducer family; `trigger_key` is an operator-facing deployment label, not the primary runtime dispatch key
- trigger rows are configuration, not detector processes; source wiring still lives in `trade-daemon`
- the stock watch families support same-symbol multi-watch fanout; `trigger_id` is the runtime identity for one deployed watch

If the user wants a new alert or background monitor:

1. inspect current triggers with `triggerctl list-trigger-definitions`
2. create or update the right trigger row with `triggerctl upsert-trigger-definition`
3. make sure `trade-daemon` is running with the needed observation source
4. inspect the resulting case with `casectl`

For stock watch alerts specifically:

- use `equity_level_watch` for fixed levels
- use `equity_vwap_bounce_watch` for touch-and-confirm bounces
- use `equity_vwap_reclaim_watch` for reclaim-after-loss behavior
- make sure `trade-daemon` is running with `TRADE_DAEMON_TWS_INTRADAY_SYMBOLS`

## Safe workflow

1. Start with `casectl list-open-case-summaries` or `casectl show-case-summary` before changing trigger rows.
2. Use `triggerctl show-default-trigger-parameters` and `triggerctl validate-trigger-parameters` before `triggerctl upsert-trigger-definition`.
3. Prefer `seed-default-trigger-definitions --apply` for baseline strategy rows instead of hand-writing every parameter.
4. If the task is operational, confirm the relevant daemon source is enabled before assuming a trigger row should fire.
5. Use the `trade-daemon` skill when the question turns into source ingestion or wake generation rather than simple inspection.

Adjustment for stock watch families:

- use `triggerctl validate-trigger-parameters` plus explicit JSON payloads
- do not assume `seed-default-trigger-definitions --apply` is the right path for those watch families

## Current smoke surfaces

DB/bootstrap smoke:

```bash
/Users/ad/work/trading-tools/scripts/smoke/smoke_db_bootstrap.sh
```

Daemon workflow smoke:

```bash
/Users/ad/work/trading-tools/scripts/smoke/smoke_daemon_workflow.sh
```

Edge wake-delivery smoke:

```bash
/Users/ad/work/trading-tools/scripts/smoke/smoke_edge_alert_flow.sh
```

## Known constraints

- the old SQLite tradedb runtime is gone
- there is no `idea create`, `idea observe`, `idea evaluate`, or `idea review add` surface anymore
- `casectl` is intentionally small: list open case summaries, show one case summary, show one case event journal
- cases are append-only from daemon transitions; operator changes usually flow through `cases.daemon_inbox`
- trigger rows are not detector instances; if the daemon source is off, nothing will fire
