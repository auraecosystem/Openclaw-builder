---
name: assistant-bot
description: >
  Minimal Discord assistant bot tooling from the mounted NautilusTrader repo.
  Use when asked to send test messages, run the mention-only listener, verify
  Discord bot token/channel setup, or troubleshoot assistant-bot command
  handling and mention behavior.
metadata: { "openclaw": { "emoji": "🤖", "requires": { "bins": ["python3"] } } }
---

# assistant-bot

Use the local non-LLM Discord assistant bot from the mounted repo at `/nautilus_trader`.

Primary doc: `/nautilus_trader/toolbox/assistant-bot/README.md`

## When to use

- "send a test message from assistant-bot"
- "run the Discord listener"
- "verify the bot only reacts when @mentioned"
- "check the bot token / channel config"
- "troubleshoot why assistant-bot did or did not answer"

## Defaults

- Run from the repo root: `cd /nautilus_trader && ...`
- Prefer the local toolbox venv when it exists:
  - `cd /nautilus_trader/toolbox/assistant-bot && source .venv/bin/activate`
- Otherwise create one and install:
  - `python3 -m venv .venv`
  - `source .venv/bin/activate`
  - `pip install -e .`
- Read config from `/nautilus_trader/toolbox/assistant-bot/.env`
- Treat `--listen` as mention-only:
  - the bot should ignore non-mentioned messages
  - the bot should handle only messages that explicitly `@mention` it

## Quick start

Basic setup:

```bash
cd /nautilus_trader/toolbox/assistant-bot
cp .env.example .env
python3 -m venv .venv
source .venv/bin/activate
pip install -e .
```

One-shot send / smoke test:

```bash
cd /nautilus_trader/toolbox/assistant-bot
source .venv/bin/activate
python bot.py --hello
python bot.py --send "hello from assistant-bot"
```

Mention-only listener:

```bash
cd /nautilus_trader/toolbox/assistant-bot
source .venv/bin/activate
python bot.py --listen
```

## Expected behavior

In `--listen` mode:

- `@assistant-bot ping` or `@assistant-bot !ping` -> reply `pong`
- `@assistant-bot alert ...` -> add a `🚨` reaction
- `@assistant-bot ping alert` -> both reply `pong` and add `🚨`
- messages without an explicit `@assistant-bot` mention -> ignore
- messages from other bots -> ignore unless the code is intentionally changed

## Actual chat command surface

The current implementation is intentionally tiny. It does not do general natural-language handling.

What it can do from chat right now:

- Reply `pong` when the mention-stripped command text starts with:
  - `ping`
  - `!ping`
- Add a `🚨` reaction when the mention-stripped command text contains:
  - `alert`
- Do both if both conditions match in the same mentioned message

Matching details:

- The bot must be explicitly mentioned with a real Discord user mention.
- The mention is stripped out before parsing.
- Matching is case-insensitive.
- Prefix check for the reply:
  - `@assistant-bot ping`
  - `@assistant-bot !ping`
  - `@assistant-bot ping now`
- Substring check for the reaction:
  - `@assistant-bot alert`
  - `@assistant-bot please alert the room`

What it cannot do from chat right now:

- no free-form Q&A
- no moderation actions
- no message delete/edit/read commands
- no slash commands
- no attachments or image handling
- no stateful conversations
- no bot-to-bot command handling
- no role-name matching; only explicit user mention metadata

## Safe workflow

1. Check `.env` first for `DISCORD_TOKEN` and `CHANNEL_ID`.
2. Use `--hello` or `--send` before leaving a long-running listener up.
3. If mention handling is the issue, test with one exact message:
   - `@assistant-bot ping`
   - `@assistant-bot alert test`
4. If the bot does not answer, verify:
   - Message Content Intent is enabled
   - the bot can view the channel and read history
   - the message actually mentioned the bot user, not a role
   - the sender was not another bot

## Known constraints

- `--listen` requires Discord Message Content Intent.
- The current implementation ignores bot-authored messages.
- The listener is command-lite on purpose; it is not a general moderation or LLM bot.
- Mention handling is based on explicit Discord mention metadata, not name matching.
- The reply path is prefix-based and the reaction path is substring-based; this is simple by design, not a full command parser.
