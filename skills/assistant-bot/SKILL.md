---
name: assistant-bot
description: >
  Use for the local Discord-native assistant-bot in the mounted NautilusTrader repo.
  Trigger when you need to operate, debug, or reason about assistant-bot command handling,
  alert registration, crypto runtime behavior, Binance Spot streaming, Discord ACK/RESULT
  responses, source-of-truth logging, or autonomous protective execution.
metadata: { "openclaw": { "emoji": "🤖", "requires": { "bins": ["python3"] } } }
---

# assistant-bot

Use the local Discord-native `assistant-bot` from `/nautilus_trader/toolbox/assistant-bot`.

Primary references:

- `/nautilus_trader/toolbox/assistant-bot/README.md`
- `/nautilus_trader/toolbox/assistant-bot/bot.py`
- `/nautilus_trader/toolbox/assistant-bot/assistant_bot_runtime/service.py`
- `/nautilus_trader/toolbox/assistant-bot/assistant_bot_runtime/runtime.py`
- `/nautilus_trader/toolbox/assistant-bot/assistant_bot_runtime/exchange.py`

When the README and code disagree, trust the code and runtime modules.

## What It Is

`assistant-bot` is no longer a tiny `ping`/`alert` Discord toy.

It is now:

- a mention-only Discord command bot
- a crypto runtime manager
- a Binance Spot market-data consumer
- an alert registry and event engine
- a protective execution controller
- a source-of-truth incident logger

For crypto-first v1, the real free venue is:

- Binance Spot public REST + websocket market data

That means:

- public candles/streaming are free
- live account/open-order state uses Binance API credentials if configured
- live execution remains gated by env flags

## When To Use

Use this skill when the task is about any of these:

- sending or listening with `assistant-bot`
- debugging why it did or did not accept a Discord command
- checking operator / `@crabman` / allowlisted-bot authorization
- verifying control-channel gating
- creating, listing, enabling, disabling, or deleting alerts
- checking `ACK` / `RESULT` message behavior
- checking event alerts or system notices
- debugging source-of-truth log records
- checking Binance Spot streaming / bootstrap / recovery behavior
- checking protective-stop sync or flatten behavior

## Runtime Defaults

- Run from repo root or toolbox dir:
  - `cd /nautilus_trader/toolbox/assistant-bot`
- Prefer the local venv:
  - `source .venv/bin/activate`
- Config lives in:
  - `/nautilus_trader/toolbox/assistant-bot/.env`
- Logs:
  - human log: `/nautilus_trader/toolbox/assistant-bot/assistant-bot.log`
  - source-of-truth JSONL: `/nautilus_trader/var/assistant-bot/source_of_truth.jsonl`
  - persisted alerts: `/nautilus_trader/var/assistant-bot/alerts.json`

## Required Discord Behavior

`assistant-bot` only handles commands when all of these are true:

- the bot is explicitly mentioned with a real user mention
- the command is inside one inline code span
- the sender is authorized
- the message is in the configured control channel/thread

Authorization rules:

- allowed humans:
  - configured operator user id
  - configured `@crabman` user id
- allowed bots:
  - only bot ids listed in `ALLOW_BOT_SENDERS`

Rejected cases include:

- no explicit bot mention
- wrong channel
- unauthorized human sender
- non-allowlisted bot sender
- malformed inline command

## Command Grammar

The live grammar is:

```text
@assistant-bot `namespace.verb key=value key2=value2`
```

Rules:

- exactly one inline code span
- command verb must be namespaced, like `market.snapshot`
- arguments must be `key=value`
- values support:
  - strings
  - ints
  - floats
  - booleans
  - `null`
  - comma-separated lists
- quoting uses shell-style quoting via `shlex`

Examples:

```text
@assistant-bot `system.status`
@assistant-bot `system.echo text="hello"`
@assistant-bot `market.snapshot symbol=BTCUSDT timeframe=1m`
@assistant-bot `alert.create symbol=BTCUSDT event_type=hard_stop_hit timeframe=1m priority=P0 barrier_level=84000 auto_actions=protective_exit`
@assistant-bot `alert.list`
@assistant-bot `alert.disable alert_id=alert_00001`
@assistant-bot `execution.sync_protection symbol=BTCUSDT quantity=0.1 side=SELL stop_price=83000`
@assistant-bot `execution.flatten symbol=BTCUSDT quantity=0.1 side=SELL reference_price=84000`
```

## Actual Command Surface

Current supported commands:

- `system.status`
- `system.echo`
- `market.snapshot`
- `alert.create`
- `alert.list`
- `alert.delete`
- `alert.enable`
- `alert.disable`
- `execution.flatten`
- `execution.sync_protection`

### `system.status`

Returns a simple runtime-health style summary.

### `system.echo`

Echoes `text=...`.

### `market.snapshot`

Required:

- `symbol`

Optional:

- `timeframe`

Returns:

- last price
- last volume
- last close time
- stale seconds

### `alert.create`

Required:

- `symbol`
- `event_type`

Optional/common:

- `timeframe`
- `priority`
- `cooldown_seconds`
- `wake_crabman`
- `auto_actions`

Everything else is treated as event condition config.

Examples:

```text
@assistant-bot `alert.create symbol=BTCUSDT event_type=hard_stop_near timeframe=1m priority=P0 barrier_level=84000 threshold_pct=0.005`
@assistant-bot `alert.create symbol=BTCUSDT event_type=hard_stop_hit timeframe=1m priority=P0 barrier_level=84000 auto_actions=protective_exit`
@assistant-bot `alert.create symbol=BTCUSDT event_type=entry_trigger_confirmed timeframe=1m priority=P1 trigger_level=84500 direction=up`
```

### `alert.list`

Returns persisted alert registrations.

### `alert.delete`

Required:

- `alert_id`

### `alert.enable` / `alert.disable`

Required:

- `alert_id`

### `execution.flatten`

Required:

- `symbol`
- `quantity`
- `reference_price`

Optional:

- `side`
- `limit_offset_pct`
- `reason`

### `execution.sync_protection`

Required:

- `symbol`
- `quantity`
- `stop_price`

Optional:

- `side`
- `limit_offset_pct`
- `reason`

## Discord Response Pattern

For accepted commands, the bot emits:

- immediate untagged `ACK`
- then a `RESULT`

Format:

```text
ACK job_00001 market.snapshot
RESULT job_00001 ok {"symbol":"BTCUSDT",...}
RESULT job_00001 error unsupported command: ...
```

Event alerts are different:

- they tag the configured operator and configured `@crabman`
- they include a compact JSON payload block

System degraded/recovered notices also tag:

- configured operator
- configured `@crabman`

## Crypto Runtime

The current crypto runtime is:

- crypto-first
- single-user / single-account
- Binance Spot-based

Market-data behavior:

- REST bootstrap for recent candles
- Binance websocket closed-candle streaming as the main feed
- one configured candle timeframe per symbol
- symbol set expands from:
  - active alerts
  - open exposure
  - active protective-order symbols

Account-state behavior:

- balances
- inferred spot exposure by symbol
- open orders
- estimated daily PnL baseline

## Current Event Families

The event engine currently supports:

- `hard_stop_near`
- `hard_stop_hit`
- `daily_loss_limit_breach`
- `position_state_mismatch`
- `data_stale_while_exposed`
- `entry_trigger_confirmed`
- `failed_breakout`
- `failed_reclaim`
- `fast_fail_post_entry`
- `fresh_reentry_trigger`

Events are bar/candle-based for a given symbol.

## Execution Behavior

Protective execution manager behavior:

- hard-stop auto action is available via `protective_exit`
- native protective stops are used when supported and enabled
- otherwise fallback is a marketable limit exit
- live execution is gated by env flags
- stop sync is idempotent and can return:
  - `created`
  - `replaced`
  - `cancel_replace`
  - `no_op`
  - `simulated`
  - `error`

Important:

- live execution is not assumed on by default
- do not claim live trading is active unless the env/config explicitly enables it

## Logging And Debugging

Two log layers matter:

- rolling human-readable log:
  - `/nautilus_trader/toolbox/assistant-bot/assistant-bot.log`
- structured source-of-truth JSONL:
  - `/nautilus_trader/var/assistant-bot/source_of_truth.jsonl`

The source-of-truth log is the main incident record. Use it for:

- command acceptance/rejection reasons
- ACK/RESULT send attempts and failures
- event triggers
- execution actions
- degraded/recovered subsystem markers
- runtime stream failures and recoveries

Common structured events to look for:

- `command.received`
- `command.ignored`
- `command.rejected`
- `command.parsed`
- `job.created`
- `job.started`
- `job.completed`
- `alert.triggered`
- `execution.completed`
- `execution.failed`
- `subsystem.degraded`
- `subsystem.recovered`
- `runtime.stream.failed`

## Safe Workflow

1. Check `.env` for:
   - `DISCORD_TOKEN`
   - `CHANNEL_ID`
   - `OPERATOR_USER_ID`
   - `CRABMAN_USER_ID`
   - `ALLOW_BOT_SENDERS`
2. For live account/execution checks, verify Binance credentials are intentionally configured.
3. Start with:
   - `python bot.py --hello`
   - or `python bot.py --send "..."`
4. Then move to:
   - `python bot.py --listen`
5. For command-path debugging, test an exact inline-code command in Discord.
6. For runtime/debug issues, inspect the source-of-truth JSONL before guessing.

## Known Constraints

- Discord Message Content Intent is required for `--listen`
- commands must be inline-code and mention-gated
- command handling is single-user/single-control-channel for v1
- `tradedb`, `TWS`, `SEC`, and equity-specific flows are deferred
- this skill is about the actual implemented bot, not the older `ping`/`alert` toy behavior
