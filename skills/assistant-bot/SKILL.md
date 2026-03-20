---
name: assistant-bot
description: >
  Use for the current Discord-only assistant-bot edge in the trading-tools
  workspace. Trigger when the task is about delivered wake alerts, Discord edge
  health, case review/operator intake through Discord, or verifying the
  assistant-bot side of the canonical wake flow. Do not use this for the old
  relay-envelope, alert-rule, or outbox architecture.
metadata: { "openclaw": { "emoji": "🤖", "requires": { "bins": ["python3"] } } }
---

# assistant-bot

Use the current assistant-bot from `/Users/ad/work/trading-tools/apps/assistant-bot`.

Primary references:

- `/Users/ad/work/trading-tools/apps/assistant-bot/README.md`
- `/Users/ad/work/trading-tools/apps/assistant-bot/src/assistant_bot/entrypoint.py`
- `/Users/ad/work/trading-tools/apps/assistant-bot/src/assistant_bot/edge/app.py`
- `/Users/ad/work/trading-tools/apps/assistant-bot/src/assistant_bot/infrastructure/delivery.py`
- `/Users/ad/work/trading-tools/var/assistant-bot.log`
- `/Users/ad/work/trading-tools/scripts/smoke/smoke_edge_alert_flow.sh`

When docs and code disagree, trust the current `assistant_bot` package code.

## What It Is

`assistant-bot` is now:

- the Discord-only edge for the canonical case engine
- a single process that consumes `wake_dispatch`
- a reader of canonical wake/case/review views from `trade_journal`
- a writer of `wake_delivery_receipt`, `wake_delivery_failure`, `review`, `case_note`, and `case_command` inbox rows

It is not:

- an alert-rule engine
- a market-data daemon
- a relay-envelope service
- an outbox processor
- the old two-process edge/daemon assistant-bot

## When To Use

Use this skill when the task is about any of these:

- verifying that a wake alert was delivered into Discord
- checking assistant-bot edge health or log output
- reading wake messages from the shared Discord channel
- confirming that a wake receipt made it back into `cases.daemon_inbox`
- using the current Discord slash-command surface as a human operator
- debugging the assistant-bot half of the daemon -> wake queue -> Discord -> daemon loop

Do not use this skill to create or manage persistent alerts. Use the canonical trigger/daemon path instead:

- `triggerctl` via the `trade-journal` skill for trigger rows
- `trade-daemon` for source ingestion and wake creation
- `assistant-bot` only for delivery and operator intake

## Core Rule

OpenClaw should treat assistant-bot as a downstream Discord edge, not as a control-plane API.

Do not assume:

- JSON relay envelopes
- `delivery.register_relay`
- `alerts.create`
- `market.quote`
- `market.bar`
- in-process imports into assistant-bot internals

Current OpenClaw pattern:

1. use the `discord` skill to read the shared Discord channel where wake alerts land
2. if a wake alert appears, inspect canonical state with `casectl` or `triggerctl`
3. if a human operator needs to act, use the Discord slash commands or write directly to the canonical inbox through the trading-tools runbooks

## Current Runtime Layout

Runbooks and code assume:

- app root: `/Users/ad/work/trading-tools/apps/assistant-bot`
- live log path is usually controlled by `ASSISTANT_BOT_LOG_PATH`
- current local smoke log path has often been `/Users/ad/work/trading-tools/var/assistant-bot.log`
- runtime entrypoint: `python -m assistant_bot`
- smoke entrypoint: `python -m assistant_bot smoke-wake-loop --once --delivery-log <path>`

## Current Command Surface

The only stable CLI/module surfaces are:

- `python -m assistant_bot --help`
- `assistant-bot`
- `assistant-bot-edge`

The current local smoke mode is:

```bash
python -m assistant_bot smoke-wake-loop --once --delivery-log /tmp/assistant-bot-wakes.jsonl
```

## Current Discord Command Surface

The live edge registers these slash commands:

- `system_status`
- `cases_list`
- `cases_show`
- `review`
- `case_note`
- `case_command`

Important:

- these are Discord slash commands, not JSON message envelopes
- OpenClaw should not claim these exist as text commands in-channel
- for bot-to-bot or scripted local smoke, use the queue/DB/smoke scripts, not fake Discord relay JSON

## Canonical Wake Flow

The supported wake path is:

1. `trade-daemon` creates a `WakeRequest`
2. `trade-daemon` enqueues `wake_dispatch`
3. `assistant-bot` reads `wake_dispatch`
4. `assistant-bot` loads the wake + case summary from `trade_journal`
5. `assistant-bot` sends the Discord wake message
6. `assistant-bot` writes `wake_delivery_receipt` or `wake_delivery_failure` to `cases.daemon_inbox`
7. `trade-daemon` consumes that inbox row and advances canonical wake state

When debugging, inspect both sides:

- `assistant-bot` log + Discord delivery
- canonical rows via `casectl show-case <case_id>` and `casectl show-events <case_id>`

## OpenClaw Usage Pattern

If OpenClaw needs to observe live alerts:

1. use the `discord` skill to read the shared Discord channel
2. look for assistant-bot wake messages
3. extract the `case_id` or symbol/summary from the wake text
4. use the `trade-journal` skill to inspect the case in `casectl`

If OpenClaw needs to help with a follow-up action:

- review the case with `casectl show-case`
- inspect trigger config with `triggerctl show` or `triggerctl list`
- inspect daemon state with the `trade-daemon` skill
- do not invent a nonexistent assistant-bot relay API

## Debugging Order

1. confirm `assistant-bot` is actually running
2. inspect the log at `/Users/ad/work/trading-tools/var/assistant-bot.log` or the path from `ASSISTANT_BOT_LOG_PATH`
3. confirm wake rows exist with `casectl show-case <case_id>` and `casectl show-events <case_id>`
4. confirm a queue message was emitted in the daemon-side smoke or DB
5. if needed, rerun `/Users/ad/work/trading-tools/scripts/smoke/smoke_edge_alert_flow.sh`

## Known Constraints

- current assistant-bot behavior is delivery-focused; it does not own trigger creation
- current OpenClaw integration is best done through a shared Discord channel, not a bot relay registration path
- the edge requires `ASSISTANT_BOT_DISCORD_TOKEN` and `ASSISTANT_BOT_DSN`
- the wake loop is queue-driven; if no wake exists, the bot has nothing to send
