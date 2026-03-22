# Assistant Bot Sidecar (OpenClaw plugin)

Runs `assistant-bot` as a gateway-managed sidecar so the bot starts with the
OpenClaw gateway, stops with it, and restarts automatically during local
development.

## Enable

Add the plugin entry to your local OpenClaw config:

```json
{
  "plugins": {
    "entries": {
      "assistant-bot-sidecar": {
        "enabled": true,
        "config": {
          "enabled": true
        }
      }
    }
  }
}
```

Restart the gateway after enabling the plugin.

## Default Behavior

By default the sidecar launches:

```bash
uv run --project /Users/ad/work/trading-tools --package assistant-bot assistant-bot
```

The sidecar:

- starts `assistant-bot` when the gateway starts
- stops `assistant-bot` when the gateway stops
- restarts the bot after unexpected exits
- watches source paths and restarts the bot after edits

Default watched paths:

- `/Users/ad/work/trading-tools/apps/assistant_bot/src`
- `/Users/ad/work/trading-tools/apps/assistant_bot/pyproject.toml`
- `/Users/ad/work/trading-tools/packages/trade_journal/src`
- `/Users/ad/work/trading-tools/packages/trade_core/src`

## Dev Loop

This plugin is meant for local development against the real `trading-tools`
checkout.

- Edit files under the watched paths and the sidecar restarts the bot.
- Restart the gateway to restart both the gateway and the bot together.
- Run the launch command directly if you want to debug `assistant-bot`
  outside the gateway lifecycle.

## Optional Config

Override the default command, working directory, restart timings, environment,
or watch paths under `plugins.entries.assistant-bot-sidecar.config`:

```json
{
  "plugins": {
    "entries": {
      "assistant-bot-sidecar": {
        "enabled": true,
        "config": {
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
          "restartDelayMs": 1000,
          "shutdownGraceMs": 5000,
          "watch": {
            "enabled": true,
            "debounceMs": 750,
            "paths": ["/Users/ad/work/trading-tools/apps/assistant_bot/src"]
          }
        }
      }
    }
  }
}
```

## Notes

- The gateway environment does not need to export the Discord token if
  `assistant-bot` loads its own `.env` file.
- This plugin is intentionally a sidecar, not a core gateway dependency, so the
  bot stays editable and runnable from its own repository.
