---
name: tradedb
description: >
  Local-first trade idea journal CLI for structured idea creation, observation
  ingestion, lifecycle evaluation, execution linkage, review capture, and FTS
  search via the standalone trading-tools workspace. Use when asked about:
  trade ideas, journaling, thesis tracking, idea state transitions,
  observations, reviews, execution linkage, or searching prior ideas.
metadata: { "openclaw": { "emoji": "🗃️", "requires": { "bins": ["uv"] } } }
---

# tradedb

Use the standalone `tradedb` CLI from `/Users/ad/work/trading-tools`.

Primary docs:

- `/Users/ad/work/trading-tools/README.md`
- `/Users/ad/work/trading-tools/packages/trade_journal/src/trade_journal/README.md`

## When to use

- "log this trade idea" / "journal this setup"
- "show my active ideas" / "find prior ideas"
- "attach this observation" / "evaluate the idea state"
- "link this order or position to an idea"
- "review this trade" / "store lessons learned"
- "search for similar theses / catalysts / reviews"

## Defaults

- Prefer the shim command: `tradedb ...`
- Fallback: `uv run --project /Users/ad/work/trading-tools tradedb ...`
- Prefer `--json` for automation or when the result will be summarized
- Preserve source event time when known:
  - use `idea observe --observed-at <iso8601>`
  - otherwise include `observed_at` in the payload if needed
- Treat `tradedb` as a journal first, not an execution engine
- Do not invent lifecycle transitions if `idea evaluate` or `position ingest` can derive them

## Quick start

Database and family setup:

```bash
tradedb db init --json
tradedb family register --file /Users/ad/work/trading-tools/packages/trade_journal/src/trade_journal/examples/families/momentum.yaml --json
```

Create and inspect ideas:

```bash
tradedb idea list --view active --json
tradedb idea show <idea-id> --json
tradedb idea find "catalyst momentum" --json
```

Observations and evaluation:

```bash
tradedb idea observe <idea-id> --source bot --observed-at 2026-03-12T14:35:00+00:00 --payload '{"approved_for_entry": true, "price": 211.2}' --json
tradedb idea evaluate --idea-id <idea-id> --json
```

Execution linkage and review:

```bash
tradedb order ingest --payload '{"external_order_id":"ord-001","venue":"SIM","account":"acct-1","side":"buy","quantity":100,"status":"new","idea_id":"<idea-id>","symbol":"AAPL"}' --json
tradedb position ingest --payload '{"venue":"SIM","account":"acct-1","net_quantity":100,"idea_id":"<idea-id>","symbol":"AAPL","average_price":211.5}' --json
tradedb idea review add <idea-id> --outcome win --score 0.8 --lessons "Catalyst worked" --text "Execution was late" --json
```

## Safe workflow

1. Start with `idea show` or `idea list` before mutating an existing idea.
2. Use `idea observe` plus `idea evaluate` for rule-driven lifecycle changes.
3. Use `transition` only for explicit operator overrides.
4. Use `position ingest` rather than manual `open` / `exited` transitions when real exposure state is known.
5. Prefer `idea find ... --json` for research or bot flows that need structured summaries.

## Known constraints

- `tradedb` is local-first SQLite state, not a broker or portfolio system.
- `idea observe` is append-only; it does not mutate prior observations.
- `order ingest` records execution linkage but does not by itself move an idea to `open`.
- `position ingest` can reconcile an idea into `open` or `exited` based on exposure.
- `--json` and `--jsonl` now emit structured error payloads for command failures, but argparse usage errors are still CLI-native.

## Output handling

- Use `--json` for anything the agent will parse, compare, or summarize.
- Use `--jsonl` only for row-streaming workflows.
- For human inspection, the default table output is fine for `list` and `find`.
