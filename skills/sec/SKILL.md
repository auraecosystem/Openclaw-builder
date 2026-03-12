---
name: sec
description: >
  SEC / EDGAR filings tooling for near-real-time 8-K and 6-K ingest, repair,
  export, and latest-feed checks via the mounted NautilusTrader toolbox. Use
  when asked about: SEC filings, EDGAR, 8-K, 6-K, filing streams, reconcile /
  backfill, latest filings, or exporting local filing state.
metadata: { "openclaw": { "emoji": "📄", "requires": { "bins": ["uv"] } } }
---

# sec

Use the NautilusTrader SEC / EDGAR CLI from the mounted repo at `/nautilus_trader`.

Primary doc: `/nautilus_trader/toolbox/sec/README.md`

## When to use

- "latest SEC filings"
- "show recent 8-Ks" / "show recent 6-Ks"
- "stream SEC filings"
- "reconcile SEC index data"
- "backfill filings for a date range"
- "export SEC filings"
- "filter filings for this CIK or company"

## Defaults

- Run from the repo root: `cd /nautilus_trader && ...`
- Prefer `uv run python -m toolbox.sec ...`
- For any network call, set a valid SEC identity:
  - `SEC_USER_AGENT="Your Name your_email@example.com"`
- Prefer `--format json` when the result will be summarized or filtered
- Use `latest` for read-only checks before starting long-running ingest
- Use `stream --once` for a low-risk smoke test before unbounded polling

## Quick start

Read-only latest feed:

```bash
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec latest --forms 8-K,6-K --format json
```

Filtered latest feed:

```bash
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec latest --forms 8-K --company NVIDIA --format json
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec latest --forms 8-K --cik 1045810 --format json
```

Polling ingest:

```bash
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec stream --once --format json
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec stream --poll-interval 60
```

Reconcile / backfill:

```bash
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec reconcile --date 2026-03-11 --format json
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec reconcile --start-date 2026-03-01 --end-date 2026-03-12 --format json
```

Export local state:

```bash
cd /nautilus_trader && uv run python -m toolbox.sec export --format json
```

## Safe workflow

1. Start with `latest --format json` for a read-only check.
2. If the feed looks useful, use `stream --once` before starting continuous polling.
3. Use `reconcile` when you need repair or historical completeness for a date or date range.
4. Use `export` after ingest/reconcile if you need parquet artifacts or downstream consumption.
5. Be explicit about `--forms` if the task is only about one filing family.

## Known constraints

- SEC network calls require a valid user-agent identity.
- `export` works from local SQLite state and does not require network access.
- `stream` is long-running unless `--once` or `--max-iterations` is set.
- This tool is focused on near-real-time 8-K / 6-K workflows, not every SEC form type.

## Output handling

- Use `--format json` for summarization, filtering, or downstream automation.
- Use table output for quick human inspection.
- For long-running polling, tell the user clearly that the command will continue until stopped.
