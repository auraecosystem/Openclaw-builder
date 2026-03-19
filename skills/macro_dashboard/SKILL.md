---
name: macro-dashboard
description: >
  Global macro dashboard and analytics workflow backed by the standalone
  trading-tools yfinance CLI. Use when asked about: macro dashboard,
  oil/gold/VIX/dollar relationships, rates/credit regime checks, global index
  context, cross-asset correlation, z-scores, volatility snapshots, or
  sectioned macro summaries.
metadata: { "openclaw": { "emoji": "🌍", "requires": { "bins": ["uv"] } } }
---

# macro-dashboard

Use the standalone `yfinance macro-dashboard` CLI from `/Users/ad/work/trading-tools`.

Primary docs:

- `/Users/ad/work/trading-tools/README.md`
- `/Users/ad/work/trading-tools/packages/trade_yahoo/src/trade_yahoo/README.md`
- `/Users/ad/work/trading-tools/packages/trade_yahoo/src/trade_yahoo/macro_dashboard.py`

## When to use

- "run the macro dashboard"
- "what does the macro dashboard say?"
- "cross-asset snapshot" / "macro regime check"
- "oil vs gold / VIX / dollar / rates"
- "global indices and risk-on / risk-off"
- "show correlations / z-scores / vol metrics"
- "just section 4" / "only give me one dashboard section"

## Defaults

- Prefer the shim command:
  - `yfinance macro-dashboard`
- Fallback:
  - `uv run --project /Users/ad/work/trading-tools yfinance macro-dashboard`
- Prefer `--json` when the result will be summarized or post-processed
- Use `--section N` when the user asks for one section or when the full dashboard would be too noisy
- Treat the output as Yahoo-derived research context, not exchange-grade execution data

## Quick start

Full dashboard:

```bash
yfinance macro-dashboard --json
```

Single section:

```bash
yfinance macro-dashboard --section 4 --json
```

## Safe workflow

1. Start with `--json` if the user wants a summary rather than raw terminal output.
2. Use `--section N` first when the user is asking about one macro slice.
3. Summarize the main cross-asset signals instead of dumping the full raw payload back.
4. If the dashboard output looks incomplete, note Yahoo coverage limits rather than inventing missing data.

## Known constraints

- The dashboard pulls Yahoo Finance data, so availability and freshness depend on Yahoo coverage.
- Some exotic or distant futures curve symbols may be stale or partially unavailable.
- This is a research dashboard, not a live trading entitlement feed.
- Large full-dashboard runs can be noisy; sectioned runs are usually easier to interpret.

## Output handling

- Use `--json` for any agent-driven summary or comparison.
- Use plain output only for quick manual inspection.
- When answering users, compress the result into the strongest macro takeaways: regime, stress markers, and notable divergences.
