---
name: stock-trading
description: >
  Umbrella workflow for discretionary and systematic stock-trading support using
  the mounted NautilusTrader repo. Use when asked to scan stocks, analyze market
  context, journal ideas, link executions, compare intraday vs swing playbooks,
  combine TWS, tradedb, and macro-dashboard outputs into one trading view, or
  render and share trading charts.
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
- `/Users/ad/work/ai/openclaw/skills/stock-charting/SKILL.md`

Deeper setup / methodology docs:

- `/Users/ad/work/ai/nautilus_trader/lab/research/INDEX.md`
- `/Users/ad/work/ai/nautilus_trader/lab/strategies/regular_session_momentum_scalper.md`
- `/Users/ad/work/ai/openclaw/skills/stock-trading/references/qullamaggie-rules.md`
- `/Users/ad/work/ai/openclaw/skills/stock-trading/references/qullamaggie-breakout.md`
- `/Users/ad/work/ai/openclaw/skills/stock-trading/references/qullamaggie-episodic-pivot.md`
- `/Users/ad/work/ai/openclaw/skills/stock-trading/references/qullamaggie-parabolic-short.md`
- `/Users/ad/work/ai/openclaw/skills/stock-trading/references/ross-cameron-rules.md`
- `/Users/ad/work/ai/openclaw/skills/stock-trading/references/ross-cameron-gap-and-go.md`
- `/Users/ad/work/ai/openclaw/skills/stock-trading/references/ross-cameron-bull-flag.md`
- `/Users/ad/work/ai/openclaw/skills/stock-trading/references/ross-cameron-flat-top-breakout.md`
- `/Users/ad/work/ai/openclaw/skills/stock-trading/references/ross-cameron-abcd.md`
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
- "plot this and send the chart"

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

### `stock-charting` for rendered charts and Discord-ready images

Use when you need:

- line or candlestick charts
- support / resistance and trend lines
- indicator overlays and lower panels
- equity curves or backtest charts
- a real PNG/JPG uploaded back to chat

Important:

- render locally
- send the file with the message tool
- do not rely on `read` as the attachment mechanism

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

Daily quick set (concise):

```bash
# 1) Active ideas
cd /nautilus_trader && uv run python -m toolbox.tradedb idea list --view active --json

# 2) Add observation
cd /nautilus_trader && uv run python -m toolbox.tradedb idea observe <idea-id> --source bot --observed-at <iso8601> --payload '{"price":123.4,"note":"..."}' --json

# 3) Re-evaluate lifecycle state
cd /nautilus_trader && uv run python -m toolbox.tradedb idea evaluate --idea-id <idea-id> --json

# 4) Link executions/positions
cd /nautilus_trader && uv run python -m toolbox.tradedb order ingest --payload '{..."idea_id":"<idea-id>"...}' --json
cd /nautilus_trader && uv run python -m toolbox.tradedb position ingest --payload '{..."idea_id":"<idea-id>"...}' --json

# 5) Add review
cd /nautilus_trader && uv run python -m toolbox.tradedb idea review add <idea-id> --outcome win|loss|scratch --score 0.0-1.0 --lessons "..." --text "..." --json

# 6) Search prior setups
cd /nautilus_trader && uv run python -m toolbox.tradedb idea find "keyword catalyst setup" --json
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
   - `/Users/ad/work/ai/openclaw/skills/stock-trading/references/ross-cameron-rules.md`
   - `/Users/ad/work/ai/openclaw/skills/stock-trading/references/ross-cameron-gap-and-go.md`
   - `/Users/ad/work/ai/openclaw/skills/stock-trading/references/ross-cameron-bull-flag.md`
   - `/Users/ad/work/ai/openclaw/skills/stock-trading/references/ross-cameron-flat-top-breakout.md`
   - `/Users/ad/work/ai/openclaw/skills/stock-trading/references/ross-cameron-abcd.md`
4. Record the thesis, risk, and trigger in `tradedb`.
5. Ingest orders / positions if the trade is taken.

### Swing breakout workflow

1. Use `macro-dashboard` to decide whether the broader regime supports breakout continuation.
2. Use `tws market bars` or existing catalog/backtest data for the name.
3. Compare the setup to:
   - `/Users/ad/work/ai/openclaw/skills/stock-trading/references/qullamaggie-rules.md`
   - `/Users/ad/work/ai/openclaw/skills/stock-trading/references/qullamaggie-breakout.md`
   - `/Users/ad/work/ai/openclaw/skills/stock-trading/references/qullamaggie-episodic-pivot.md`
   - `/Users/ad/work/ai/openclaw/skills/stock-trading/references/qullamaggie-parabolic-short.md`
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

## Universal guardrails

Apply these to any strategy or playbook unless the user explicitly overrides them.

- Enter on confirmation, not anticipation.
  Do not buy blind catches, early knife-catches, or unconfirmed breakouts just because price is lower or moving fast.
- Define invalidation before entry.
  Every trade should have a clear "I am wrong" line plus hard-stop and soft-stop behavior.
- Use fixed-fraction risk and stable sizing.
  Risk per trade should be pre-sized from stop distance; do not increase size because emotions rise or the trade is under water.
- Never average down into a loser.
  Adds are only for improved confirmation or a fresh trigger, not for loss repair.
- Exit failed momentum quickly.
  If the expected extension does not happen, or reclaim/breakout structure fails, reduce or exit without debate.
- Re-enter only on a new setup.
  A stopped-out trade may be re-entered only on a fresh trigger with a fresh plan, not by repairing the prior mistake.
- Respect daily loss and cool-off controls.
  Use max daily loss, max consecutive-loss, and post-stop cooldown rules to prevent tilt and size-creep behavior.
- Check regime and data quality first.
  Reduce size or do not trade when the macro tape, liquidity, spread/volume quality, or catalyst quality is hostile or unreliable.
- Log thesis, trigger, invalidation, and risk in `tradedb`.
  Treat journaling as part of trade approval, not post-hoc cleanup.
- Separate execution quality from thesis quality.
  A good thesis with bad execution is still a bad trade; record both explicitly in reviews.

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

## Equity curve benchmarking (0% to 5% daily bands)

Use this when the user wants to compare live account progress against fixed daily compounding scenarios (for example 0%, 1%, 2%, 3%, 4%, 5%).

1. Pull current account value from TWS (prefer NetLiquidation):

```bash
cd /nautilus_trader && uv run tws --format json account overview
```

2. Render multi-line projection chart with log Y-axis using Python tooling.

Reference script pattern (adapt start value/date/rates as requested):

```bash
python3 scripts/compound_projection_multi.py   --start-value <latest_net_liq>   --start-date YYYY-MM-DD   --end-date YYYY-MM-DD   --rates 0.00 0.01 0.02 0.03 0.04 0.05   --output compound_projection_multi_0to5pct_logy.png
```

3. Share the chart back to chat as a real media attachment/path (not plain text).

Notes:

- Keep all rates on the same figure.
- Prefer log Y-axis so early and late periods are both legible.
- Include title, axis labels, legend, and grid.
- If requested, overlay an observed live equity series on top of the scenario bands.

## Defaults

- Prefer `--json` whenever outputs will be summarized or chained into another tool step.
- Do not place or modify broker orders unless the user explicitly asks.
- Preserve source event time in `tradedb` observations with `--observed-at` when known.
- Use `sec` when a thesis depends on a fresh filing rather than only headlines or broker news.
- For live market data, prefer read-only and delayed-safe queries first.
- If the task is narrow and only one tool is needed, use that narrower tool directly instead of forcing the whole workflow.
