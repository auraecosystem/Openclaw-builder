---
name: assistant-bot
description: >
  Use for the current Discord-native assistant-bot in the trading-tools
  workspace. Trigger when you need to create, inspect, pause, resume, or delete
  alerts through Discord; send bot relay envelopes to assistant-bot; register
  or debug OpenClaw relay delivery; inspect edge/daemon/Postgres health; or
  reason about the current layered alert service. Do not use this for the old
  execution/protective-stop assistant-bot behavior.
metadata: { "openclaw": { "emoji": "🤖", "requires": { "bins": ["python3"] } } }
---

# assistant-bot

Use the current assistant-bot from `/Users/ad/work/trading-tools/apps/assistant-bot`.

Primary references:

- `/Users/ad/work/trading-tools/apps/assistant-bot/README.md`
- `/Users/ad/work/trading-tools/docs/architecture/explanation-assistant-bot-discord-architecture.md`
- `/Users/ad/work/trading-tools/docs/architecture/reference-assistant-bot-local-operations.md`
- `/Users/ad/work/trading-tools/apps/assistant-bot/src/assistant_bot/edge/app.py`
- `/Users/ad/work/trading-tools/apps/assistant-bot/src/assistant_bot/daemon/app.py`
- `/Users/ad/work/trading-tools/apps/assistant-bot/src/assistant_bot/infrastructure/delivery.py`

When docs and code disagree, trust the current `assistant_bot` package code.

## What It Is

`assistant-bot` is now:

- a layered Discord alert service
- a two-process system:
  - `assistant-bot-edge` for slash commands, bot relay intake, and Discord delivery
  - `assistant-bot-daemon` for polling, evaluation, cooldown/dedupe, and outbox writes
- a Postgres-backed operational service when `ASSISTANT_BOT_DSN` is configured
- alerts-and-notifications only

It is not:

- an execution bot
- a Binance-protection manager
- a single-user control-channel bot
- the old `assistant_bot_runtime` architecture

## When To Use

Use this skill when the task is about any of these:

- creating or managing market alerts through Discord
- making OpenClaw ask assistant-bot for quotes, bars, or status
- registering or using a bot relay target such as `bot:openclaw`
- debugging why assistant-bot did or did not answer in Discord
- checking edge, daemon, Postgres, outbox, audit, or delivery behavior
- checking current providers, current alert conditions, or current relay envelope shape

## Core Rule

OpenClaw must use assistant-bot the same way any other Discord client does.

Do not assume:

- in-process hooks
- local Python imports into assistant-bot internals
- privileged execution commands
- legacy `ACK` then `RESULT` behavior

Preferred control path:

1. send a Discord message to assistant-bot using the `discord` skill and the `message` tool
2. mention assistant-bot at the start of the message
3. send a JSON relay envelope
4. wait for the `command.result` envelope reply

## Current Transport Model

Humans use slash commands.

Bots use mention-scoped JSON relay messages.

OpenClaw should use the bot relay path.

Current local assistant-bot bot user:

- assistant-bot Discord user id: `1482046674519199826`

Current local allowlisted OpenClaw bot user:

- OpenClaw Discord user id: `1475800944619814943`

Current local smoke-test guild:

- guild id: `1475801759485005895`

## Relay Envelope

The relay request format is:

```text
<@1482046674519199826> {"v":1,"request_id":"req-1","op":"system.status","args":{}}
```

Rules:

- mention assistant-bot first
- body must be valid JSON
- `v` must be `1`
- `request_id` must be a non-empty unique string
- `op` must be a supported operation
- `args` must be an object

Response format:

```text
<@1475800944619814943> {"v":1,"kind":"command.result","request_id":"req-1","ok":true,"message":"..."}
```

Alert-delivery relay format:

```text
<@1475800944619814943> {"v":1,"kind":"alert.fired","event_id":"...","rule_id":"...","instrument":{"asset_class":"crypto","venue":"binance_spot","symbol":"BTCUSDT","timeframe":"1m"},"fact":{"type":"bar_closed","close":83990.0,"open":84020.0,"high":84050.0,"low":83970.0,"volume":1200.0,"cursor":"BTCUSDT:1m:..."},"triggered_at":"2026-03-19T16:01:00Z"}
```

## Supported Relay Ops

Current supported bot relay operations:

- `alerts.create`
- `alerts.list`
- `alerts.show`
- `alerts.pause`
- `alerts.resume`
- `alerts.delete`
- `alerts.destinations`
- `alerts.test_delivery`
- `market.quote`
- `market.bar`
- `system.status`
- `delivery.register_relay`
- `watchlist.add`
- `watchlist.remove`
- `watchlist.list`

## Supported Alert Model

Current providers:

- `crypto/binance_spot`
- `crypto/coinbase_spot`
- `equity/yahoo_equities`

Current condition types:

- `price_above`
- `price_below`
- `price_cross`
- `percent_move`
- `volume_above`
- `volume_spike`
- `relative_volume`
- `data_stale`

Important:

- there are no `execution.flatten` or `execution.sync_protection` commands anymore
- there is no specialized legacy event catalog anymore
- assistant-bot now uses generic alert rules only

## OpenClaw Usage Pattern

Use the `discord` skill and the `message` tool.

Minimal status check:

```json
{
  "action": "send",
  "channel": "discord",
  "to": "channel:<assistant-bot-channel-id>",
  "message": "<@1482046674519199826> {\"v\":1,\"request_id\":\"status-1\",\"op\":\"system.status\",\"args\":{}}",
  "silent": true
}
```

Quote request example:

```json
{
  "action": "send",
  "channel": "discord",
  "to": "channel:<assistant-bot-channel-id>",
  "message": "<@1482046674519199826> {\"v\":1,\"request_id\":\"quote-1\",\"op\":\"market.quote\",\"args\":{\"instrument\":{\"asset_class\":\"equity\",\"venue\":\"yahoo_equities\",\"symbol\":\"NVDA\"}}}",
  "silent": true
}
```

Create alert example:

```json
{
  "action": "send",
  "channel": "discord",
  "to": "channel:<assistant-bot-channel-id>",
  "message": "<@1482046674519199826> {\"v\":1,\"request_id\":\"alert-1\",\"op\":\"alerts.create\",\"args\":{\"delivery_target\":\"bot:openclaw\",\"instrument\":{\"asset_class\":\"equity\",\"venue\":\"yahoo_equities\",\"symbol\":\"NVDA\",\"timeframe\":\"1d\"},\"condition\":{\"type\":\"price_above\",\"value\":1000},\"cooldown_seconds\":3600}}",
  "silent": true
}
```

Safe smoke-test alert:

- use a value that should not fire accidentally
- for example `price_below=1` on `BTCUSDT`

## Relay Target Setup

OpenClaw can only receive alert deliveries through assistant-bot after a relay target exists.

Preferred target id:

- `bot:openclaw`

Register it once with either:

- the slash command `/system register_relay_target`
- or the relay op `delivery.register_relay`

Example relay-target registration:

```text
<@1482046674519199826> {"v":1,"request_id":"register-openclaw-1","op":"delivery.register_relay","args":{"target_id":"bot:openclaw","bot_user_id":"1475800944619814943","label":"openclaw","shared":true}}
```

After that, use:

- `delivery_target="bot:openclaw"` in `alerts.create`

## Current Runtime Layout

Runbooks and code assume:

- app root: `/Users/ad/work/trading-tools/apps/assistant-bot`
- config: `/Users/ad/work/trading-tools/apps/assistant-bot/.env`
- app log: `/Users/ad/work/trading-tools/apps/assistant-bot/var/assistant-bot.log`
- local Postgres container: `assistant-bot-postgres`

Operational checks:

- edge process logs into Discord and syncs slash commands
- daemon process polls providers and writes outbox rows
- Postgres stores alert rules, runtime state, outbox, audit, idempotency, delivery targets, and watchlists

## Debugging Order

1. check assistant-bot health with `system.status`
2. check that assistant-bot answered with a `command.result`
3. check whether the relay target exists with `alerts.destinations`
4. check the app log at `/Users/ad/work/trading-tools/apps/assistant-bot/var/assistant-bot.log`
5. if needed, inspect Postgres-backed state using the local operations runbook

If slash commands are missing:

- check `ASSISTANT_BOT_GUILD_ID`
- restart `assistant-bot-edge`
- verify the edge log contains `synced 4 application commands to guild 1475801759485005895`

## Known Constraints

- current bot relay parsing depends on Discord message content being available
- assistant-bot requires Discord scopes `bot` and `applications.commands`
- current edge intents are `Guilds`, `Guild Messages`, and `Message Content`
- the relay path only accepts allowlisted bot senders
- assistant-bot is a monitoring and notification service, not an execution service
