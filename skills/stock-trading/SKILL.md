---
name: stock-trading
description: >
  Umbrella workflow for discretionary and systematic stock-trading support using
  the mounted NautilusTrader repo. Use when asked to scan stocks, analyze market
  context, journal ideas, link executions, compare intraday vs swing playbooks,
  or combine TWS, tradedb, and macro-dashboard outputs into one trading view.
metadata: { "openclaw": { "emoji": "📈", "requires": { "bins": ["uv"] } } }
---

# stock-trading

Use this as the top-level stock trading workflow when the task spans more than one tool or needs a full trading process:

- discovery and live market context
- macro regime context
- idea journaling and lifecycle tracking
- execution linkage and review
- playbook lookup for deeper setup context

Primary tool docs:

- `/nautilus_trader/toolbox/tws/README.md`
- `/nautilus_trader/toolbox/tradedb/README.md`
- `/nautilus_trader/toolbox/yfinance/README.md`
- `/nautilus_trader/toolbox/yfinance/macro_dashboard.py`
- `/nautilus_trader/toolbox/sec/README.md`

Deeper setup / methodology docs:

- `/Users/ad/work/ai/nautilus_trader/lab/research/INDEX.md`
- `/Users/ad/work/ai/nautilus_trader/lab/strategies/regular_session_momentum_scalper.md`
- `/Users/ad/work/ai/nautilus_trader/lab/research/markets/qullamaggie-rules.md`
- `/Users/ad/work/ai/openclaw/skills/algotrader/references/cameron-rules.md`
- `/Users/ad/work/ai/openclaw/skills/algotrader/references/strategy-comparison.md`
- `/Users/ad/work/ai/nautilus_trader/lab/research/markets/2026-03-12_macro-dashboard-news-context.md`

## When to use

- "scan for stocks and tell me what matters"
- "build a trade plan"
- "log this idea and track it"
- "how does macro affect this setup?"
- "compare a Qullamaggie swing versus a Ross Cameron intraday trade"
- "show me candidate longs/shorts and record the thesis"
- "link this order / position / review to the idea"

## Workflow

1. Establish macro and market regime first.
2. Run discovery / live checks for symbols or sectors.
3. Decide the trade style: intraday momentum, swing breakout, event-driven, or no trade.
4. Record the idea in `tradedb` before or alongside execution.
5. After execution, ingest order / position state and later add a review.

## Tool selection

### `tws` for live discovery and broker-facing market data

Use when you need:

- scanners
- quote snapshots
- bars from IBKR
- positions / executions / contracts / options / news

Default entrypoint:

```bash
cd /nautilus_trader && uv run tws ...
```

Common examples:

```bash
cd /nautilus_trader && uv run tws scanner run --scan-code TOP_PERC_GAIN
cd /nautilus_trader && uv run tws market snapshot AAPL --delayed
cd /nautilus_trader && uv run tws market bars AAPL NVDA --duration "30 D" --bar-size "1 day" --what-to-show TRADES
cd /nautilus_trader && uv run tws account positions
```

### `macro-dashboard` for cross-asset regime context

Use when you need:

- oil / gold / dollar / VIX context
- yield / credit / EM stress proxies
- correlation, z-score, vol, and geopolitical overlays
- a regime filter before taking equity trades

Default entrypoint:

```bash
cd /nautilus_trader && uv run --with yfinance python -m toolbox.yfinance macro-dashboard --json
```

Targeted section:

```bash
cd /nautilus_trader && uv run --with yfinance python -m toolbox.yfinance macro-dashboard --section 9 --json
```

### `tradedb` for idea journaling and execution linkage

Use when you need:

- idea creation
- observations and lifecycle evaluation
- search across prior theses, reviews, and execution payloads
- order / position linkage
- outcome review

Default entrypoint:

```bash
cd /nautilus_trader && uv run python -m toolbox.tradedb ...
```

Common examples:

```bash
cd /nautilus_trader && uv run python -m toolbox.tradedb idea list --view active --json
cd /nautilus_trader && uv run python -m toolbox.tradedb idea observe <idea-id> --source bot --observed-at 2026-03-12T14:35:00+00:00 --payload '{"price": 211.2, "approved_for_entry": true}' --json
cd /nautilus_trader && uv run python -m toolbox.tradedb idea evaluate --idea-id <idea-id> --json
cd /nautilus_trader && uv run python -m toolbox.tradedb order ingest --payload '{"external_order_id":"ord-001","venue":"SIM","account":"acct-1","side":"buy","quantity":100,"status":"new","idea_id":"<idea-id>","symbol":"AAPL"}' --json
```

### `sec` for SEC / EDGAR filing intake and repair

Use when you need:

- recent 8-K / 6-K filings
- SEC filing checks for a symbol / company / CIK
- filing streams and reconcile / repair
- local export of filing state for downstream analysis

Default entrypoint:

```bash
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec ...
```

Common examples:

```bash
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec latest --forms 8-K,6-K --format json
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec latest --forms 8-K --company NVIDIA --format json
cd /nautilus_trader && SEC_USER_AGENT="Your Name your_email@example.com" uv run python -m toolbox.sec reconcile --date 2026-03-11 --format json
cd /nautilus_trader && uv run python -m toolbox.sec export --format json
```

## How to combine them

### Intraday momentum workflow

1. Use `macro-dashboard` first to decide whether the tape is supportive, mixed, or hostile for momentum.
2. Use `tws scanner` and `market snapshot` / `market bars` to build a watchlist.
3. Compare the candidates to the intraday playbook in:
   - `/Users/ad/work/ai/nautilus_trader/lab/strategies/regular_session_momentum_scalper.md`
   - `/Users/ad/work/ai/openclaw/skills/algotrader/references/cameron-rules.md`
4. Record the thesis, risk, and trigger in `tradedb`.
5. Ingest orders / positions if the trade is taken.

### Swing breakout workflow

1. Use `macro-dashboard` to decide whether the broader regime supports breakout continuation.
2. Use `tws market bars` or existing catalog/backtest data for the name.
3. Compare the setup to:
   - `/Users/ad/work/ai/nautilus_trader/lab/research/markets/qullamaggie-rules.md`
   - `/Users/ad/work/ai/openclaw/skills/algotrader/references/strategy-comparison.md`
4. Log the idea in `tradedb` before execution.
5. Review outcome quality later in `tradedb`.

### Event / catalyst workflow

1. Use `tws news` and scanner output to identify the catalyst name.
2. Use `sec latest` or `sec reconcile` when the catalyst may be tied to a fresh filing.
3. Use `macro-dashboard` to determine whether the catalyst is fighting or aligned with the broader tape.
4. Use `tradedb` to record:
   - thesis
   - catalyst
   - invalidation
   - execution linkage
5. If needed, compare the setup to intraday or swing playbooks before choosing the holding period.

## Decision heuristics

- Use intraday momentum playbooks when:
  - the move is catalyst-driven now
  - liquidity and volume are concentrated in the open
  - the edge depends on speed, HOD, VWAP, and opening-range behavior

- Use swing playbooks when:
  - the move is building over days to weeks
  - market regime and trend alignment matter more than minute-to-minute tape
  - the setup resembles EP / breakout / parabolic structures rather than a morning scalp

- Use macro data mostly as:
  - a filter
  - a sizing input
  - a hold-time input
    not as the exact entry trigger

## Defaults

- Prefer `--json` whenever outputs will be summarized or chained into another tool step.
- Do not place or modify broker orders unless the user explicitly asks.
- Preserve source event time in `tradedb` observations with `--observed-at` when known.
- Use `sec` when a thesis depends on a fresh filing rather than only headlines or broker news.
- For live market data, prefer read-only and delayed-safe queries first.
- If the task is narrow and only one tool is needed, use that narrower tool directly instead of forcing the whole workflow.
