# Assistant Bot Sidecar (OpenClaw plugin)

Runs `trade-daemon` and `assistant-bot` as a gateway-managed local stack so the
daemon starts first, the bot starts after it, and both processes restart in a
clean order during development.

## Enable

Add the plugin entry to your local OpenClaw config:

```json
{
  "plugins": {
    "entries": {
      "assistant-bot-sidecar": {
        "enabled": true,
        "config": {
          "enabled": true,
          "startupGraceMs": 1500
        }
      }
    }
  }
}
```

Restart the gateway after enabling the plugin.

The sidecar loads database/runtime settings from env files instead of duplicating
host and port values inline in the OpenClaw config.

## Default Behavior

By default the sidecar launches:

```bash
uv run --project /Users/ad/work/trading-tools --package trade-daemon trade-daemon run
uv run --project /Users/ad/work/trading-tools --package assistant-bot assistant-bot
```

The sidecar:

- starts `trade-daemon` first
- waits for the daemon startup grace window before starting `assistant-bot`
- stops `assistant-bot` before stopping `trade-daemon`
- restarts only `assistant-bot` after bot-only edits or bot crashes
- restarts `trade-daemon` and then `assistant-bot` after daemon/shared edits or daemon crashes

Default env files:

- daemon:
  `/Users/ad/work/trading-tools/.env`
- assistant bot:
  `/Users/ad/work/trading-tools/.env`
- assistant bot:
  `/Users/ad/work/trading-tools/apps/assistant_bot/.env`

The plugin maps shared `TRADING_TOOLS_TIMESCALE_*` values from
`/Users/ad/work/trading-tools/.env` onto the `TRADE_DB_*` vars expected by the
Python services. `assistant-bot` can still override its own DSN in
`apps/assistant_bot/.env`.

Default watched paths:

- daemon:
  `/Users/ad/work/trading-tools/apps/trade_daemon/src`
- daemon:
  `/Users/ad/work/trading-tools/apps/trade_daemon/pyproject.toml`
- daemon:
  `/Users/ad/work/trading-tools/packages/trade_strategies/src`
- assistant bot:
  `/Users/ad/work/trading-tools/apps/assistant_bot/src`
- assistant bot:
  `/Users/ad/work/trading-tools/apps/assistant_bot/pyproject.toml`
- shared:
  `/Users/ad/work/trading-tools/packages/trade_core/src`
- shared:
  `/Users/ad/work/trading-tools/packages/trade_journal/src`

## Dev Loop

This plugin is meant for local development against the real `trading-tools`
checkout.

- Edit `assistant_bot` files to restart only the bot.
- Edit `trade_daemon`, `trade_strategies`, `trade_core`, or `trade_journal`
  files to restart the daemon first and then the bot.
- Restart the gateway to restart the whole supervised stack.
- Run either launch command directly if you want to debug one process outside
  the gateway lifecycle.

## Optional Config

Override commands, env files, environment, restart timings, or watch paths under
`plugins.entries.assistant-bot-sidecar.config`:

```json
{
  "plugins": {
    "entries": {
      "assistant-bot-sidecar": {
        "enabled": true,
        "config": {
          "enabled": true,
          "startupGraceMs": 1500,
          "daemon": {
            "enabled": true,
            "cwd": "/Users/ad/work/trading-tools",
            "command": "uv",
            "args": [
              "run",
              "--project",
              "/Users/ad/work/trading-tools",
              "--package",
              "trade-daemon",
              "trade-daemon",
              "run"
            ],
            "envFiles": ["/Users/ad/work/trading-tools/.env"],
            "restartDelayMs": 1000,
            "shutdownGraceMs": 5000,
            "watch": {
              "enabled": true,
              "debounceMs": 750,
              "paths": [
                "/Users/ad/work/trading-tools/apps/trade_daemon/src",
                "/Users/ad/work/trading-tools/apps/trade_daemon/pyproject.toml",
                "/Users/ad/work/trading-tools/packages/trade_strategies/src"
              ]
            }
          },
          "assistantBot": {
            "enabled": true,
            "cwd": "/Users/ad/work/trading-tools",
            "command": "uv",
            "args": [
              "run",
              "--project",
              "/Users/ad/work/trading-tools",
              "--package",
              "assistant-bot",
              "assistant-bot"
            ],
            "envFiles": [
              "/Users/ad/work/trading-tools/.env",
              "/Users/ad/work/trading-tools/apps/assistant_bot/.env"
            ],
            "restartDelayMs": 1000,
            "shutdownGraceMs": 5000,
            "watch": {
              "enabled": true,
              "debounceMs": 750,
              "paths": [
                "/Users/ad/work/trading-tools/apps/assistant_bot/src",
                "/Users/ad/work/trading-tools/apps/assistant_bot/pyproject.toml"
              ]
            }
          },
          "sharedWatchPaths": [
            "/Users/ad/work/trading-tools/packages/trade_core/src",
            "/Users/ad/work/trading-tools/packages/trade_journal/src"
          ]
        }
      }
    }
  }
}
```

## Notes

- Keep DB host, port, user, and password in env files rather than the OpenClaw
  plugin config.
- `assistant-bot` can still load its Discord token from its own `.env` file.
- This remains a plugin-side sidecar, not a gateway core dependency, so the
  trading stack stays editable and runnable from `trading-tools`.
