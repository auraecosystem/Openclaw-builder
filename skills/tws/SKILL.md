---
name: tws
description: >
  Interactive Brokers / TWS CLI for market data, scanners, account state,
  contracts, options, depth, news, and momentum workflows via the standalone
  trading-tools workspace. Use when asked about: IBKR, TWS, Gateway, stock
  scans, historical bars, quote snapshots, delayed quotes, positions,
  executions, option chains, option greeks, market depth, or news.
metadata: { "openclaw": { "emoji": "📡", "requires": { "bins": ["uv"] } } }
---

# tws

Use the standalone `tws` CLI from `/Users/ad/work/trading-tools`.

Primary docs:

- `/Users/ad/work/trading-tools/README.md`
- `/Users/ad/work/trading-tools/packages/trade_tws/src/trade_tws/README.md`

## When to use

- "scan stocks" / "top gainers" / "premarket movers"
- "get IBKR bars" / "historical bars" / "quote snapshot"
- "show my positions" / "account overview" / "recent executions"
- "resolve this contract" / "option chain" / "option greeks"
- "news providers" / "historical headlines"
- "momentum scan" / "scanner run"

## Defaults

- Prefer the shim command: `tws ...`
- Fallback: `uv run --project /Users/ad/work/trading-tools tws ...`
- Prefer delayed quotes unless the user explicitly needs live data:
  - add `--delayed` for snapshot/watch quote commands when appropriate
- The last verified live TWS setup used port `7496`, while the CLI default is `7497`
  - if a command unexpectedly fails to connect, retry with `--port 7496`
- Favor read-only / observational commands first
- Do not place or modify orders unless the user explicitly asks

## Quick start

Connectivity / low-risk checks:

```bash
tws account overview
tws contracts resolve AAPL
tws market snapshot AAPL --delayed
```

Scanner:

```bash
tws scanner params
tws scanner run --scan-code TOP_PERC_GAIN
```

Historical market data:

```bash
tws market bars AAPL MSFT --duration "30 D" --bar-size "1 day" --what-to-show TRADES
tws market ticks trades AAPL --end "20260312 13:26:50 US/Eastern" --num-ticks 10
```

Account / executions:

```bash
tws account summary
tws account positions
tws account pnl
tws executions list --limit 20
```

Options:

```bash
tws options chain AAPL
tws options quote AAPL --expiry 20260320 --strike 255 --right C
tws options greeks AAPL --expiry 20260320 --strike 255 --right C
```

News:

```bash
tws news providers
tws news history --symbol AAPL --limit 5
```

## Safe workflow

1. Start with `contracts resolve` if the symbol/instrument may be ambiguous.
2. For quote visibility, prefer `market snapshot --delayed` before long-running watchers.
3. For historical data, prefer `market bars` over streaming commands.
4. For options, always use a fully specified contract: symbol, expiry, strike, right.
5. For account questions, prefer `account overview`, then drill into `positions`, `pnl`, or `executions`.

## Known constraints

- US equity live streaming quotes may be blocked on the current account; delayed quotes are the safe fallback.
- `reqRealTimeBars` is currently blocked on the verified account.
- Option and market-data availability depends on the logged-in IBKR account entitlements.
- The CLI also supports legacy flat aliases, but prefer the namespaced commands from the README.

## Output handling

- Use `--format json` when the result needs to be post-processed or summarized programmatically.
- Use table output for quick human-readable inspection.
- For repeated polling, be explicit that the command is long-running (`account pnl --watch`, `market watch quotes ...`).
