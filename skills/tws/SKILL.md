---
name: tws
description: >
  Interactive Brokers / TWS CLI for market data, scanners, account state,
  order entry, contracts, options, depth, news, and momentum workflows via the
  standalone trading-tools workspace. Use when asked about: IBKR, TWS,
  Gateway, stock scans, historical bars, quote snapshots, delayed quotes,
  positions, executions, order entry, option chains, option greeks, market
  depth, or news.
metadata: { "openclaw": { "emoji": "📡", "requires": { "bins": ["uv"] } } }
---

# tws

Use the standalone `tws` CLI from `/Users/ad/work/trading-tools`.

Primary docs:

- `/Users/ad/dotfiles/docs/tools/tws/reference/tws-cli-reference.md`
- `/Users/ad/dotfiles/docs/domains/trading/research/dated/2026-03-25-tws-cli-paper-surface-probe.md`
- `/Users/ad/work/trading-tools/README.md`
- `/Users/ad/work/trading-tools/packages/trade_tws/src/trade_tws/README.md`

## When to use

- "scan stocks" / "top gainers" / "premarket movers"
- "get IBKR bars" / "historical bars" / "quote snapshot"
- "show my positions" / "account overview" / "recent executions"
- "buy this stock" / "sell this stock" / "preview an order" / "cancel order"
- "resolve this contract" / "option chain" / "option greeks"
- "news providers" / "historical headlines"
- "momentum scan" / "scanner run"

## Defaults

- Prefer the shim command: `tws ...`
- Fallback: `uv run --project /Users/ad/work/trading-tools tws ...`
- On this Mac, `/Users/ad/bin/tws` currently exports `TWS_PORT=7497` when unset, so bare `tws ...` targets the paper TWS session by default
  - order-entry commands (`buy`, `sell`, `orders open`, `orders completed`, `orders cancel`) intentionally ignore that shim default and use `7496` unless you pass `--port`
  - use `tws --port 7496 ...` for the live TWS session on read-only commands
- Prefer delayed quotes unless the user explicitly needs live data:
  - add `--delayed` for snapshot/watch quote commands when appropriate
- Global options must come before the subcommand:
  - use `tws --format json scanner run ...`, not `tws scanner run ... --format json`
- If TWS returns error `326` ("client id is already in use"), retry with a unique client id:
  - for example `tws --client-id 201 --format json scanner run ...`
- For `news history`, prefer explicit provider codes such as `--provider-codes DJ-N`
  - omitting provider codes can fail with IBKR error `321` on the paper session
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
tws scanner params --filter ScanCode
tws --client-id 201 --format json scanner run --scan-code TOP_PERC_GAIN --num-rows 10
tws --client-id 202 --format json scanner run --scan-code HOT_BY_VOLUME --num-rows 10
```

Historical market data:

```bash
tws market bars AAPL MSFT --duration "30 D" --bar-size "1 day" --what-to-show TRADES
tws market ticks trades AAPL --end "20260312 13:26:50 US/Eastern" --num-ticks 10
# IBKR crypto needs explicit contract fields + AGGTRADES
tws --port 7496 market bars ETH --sec-type CRYPTO --exchange PAXOS --currency USD --duration "2 D" --bar-size "1 hour" --what-to-show AGGTRADES --include-eth
```

Account / executions:

```bash
tws account summary
tws account positions
tws account pnl
tws executions list --limit 20
```

Trading:

```bash
tws buy AAPL --quantity 10 --order-type limit --limit-price 101.25 --account DU123456
tws sell AAPL --quantity 5 --order-type market --what-if
tws orders open
tws orders completed --limit 20
tws orders cancel 12345678
```

- `buy` / `sell`: submit live orders. Supported `--order-type` values are `market`, `limit`, `stop`, `stop_limit`, `trailing_stop`, `trailing_stop_limit`, `market_on_close`, `limit_on_close`, `market_if_touched`, and `limit_if_touched`.
- `--what-if`: preview the order without transmitting it.
- `--no-transmit`: build the order chain without transmitting it.
- `--take-profit` and `--stop-loss`: build simple bracket exits.
- `orders open`: account-wide open-order view.
- `orders completed`: completed-order history from IBKR's completed-order API.
- `executions list`: fills history; keep it separate from completed-order history.

Options:

```bash
tws options chain AAPL
tws options quote AAPL --expiry 20260320 --strike 255 --right C
tws options greeks AAPL --expiry 20260320 --strike 255 --right C
```

News:

```bash
tws news providers
tws news history --symbol AAPL --provider-codes DJ-N --limit 5
```

Indicators:

```bash
tws indicators AAPL --duration "1 M"
```

## Verified paper-session surface (2026-03-25)

Working one-shot commands:

- `scanner params`, `scanner run`, `session-movers`
- `market bars`, `history`, `market ticks trades`
- `account summary`, `account positions`, `account pnl`, `account overview`, `orders open`, `executions list`
- `contracts search`, `contracts resolve`, `contracts market-rule`
- `options chain`, `options resolve`, `options quote`, `options greeks`, `options bars`, `options quote-history`, `options quote-ticks`
- `depth exchanges`, `news providers`, `news history --provider-codes DJ-N`, `news article`, `indicators`

Implemented but not clean one-shot surfaces:

- `market snapshot` and legacy `snapshot` emitted delayed quote payloads on port `7497` but did not terminate cleanly in the short probe window
- `market watch quotes` and `momentum` are long-running commands; use them intentionally and be ready to interrupt

Blocked or entitlement-limited on the paper session:

- `observe` failed with IBKR error `10168` for delayed market-data entitlement
- legacy `watch --mode bars` failed with IBKR error `420` for real-time market-data permissions

## Safe workflow

1. Start with `contracts resolve` if the symbol/instrument may be ambiguous.
2. For quote visibility, prefer `market snapshot --delayed` before long-running watchers.
3. For historical data, prefer `market bars` over streaming commands.
4. For options, always use a fully specified contract: symbol, expiry, strike, right.
5. For account questions, prefer `account overview`, then drill into `positions`, `pnl`, or `executions`.
6. For stock scanners, keep the default `--location STK.US.MAJOR` unless you intentionally want broader coverage.

## Known constraints

- US equity live streaming quotes may be blocked on the current account; delayed quotes are the safe fallback.
- `reqRealTimeBars` is currently blocked on the verified account.
- Option and market-data availability depends on the logged-in IBKR account entitlements.
- On the verified setup, `STK.US` can trigger IBKR error `492` for scanner permissions involving Pink Sheets, while `STK.US.MAJOR` returns results.
- On the local paper-session probe, `news history` without explicit `--provider-codes` failed with IBKR error `321`.
- **IBKR crypto requires explicit contract fields**: use `--sec-type CRYPTO --exchange PAXOS --currency USD` instead of stock defaults.
- **IBKR crypto historical bars require `AGGTRADES`**, not `TRADES`; otherwise IBKR returns error `10299`.
- **IBKR crypto on PAXOS is not actually live on Saturday in this setup**. Contract details reported Saturday closed and the next session opening Sunday; do not assume 24/7 weekend ticks are available.
- The CLI also supports legacy flat aliases, but prefer the namespaced commands from the README.

## Client ID and order lifecycle

- Treat `--client-id` as the identity of a single API session/connection, not as an account or order identifier.
- Use a **unique client ID per concurrently connected API client**; reusing a live client ID can trigger IBKR error `326` (`client id is already in use`).
- Keep one stable client ID for the process that owns order lifecycle actions (place / modify / cancel) for a given strategy.
- `orderId` is scoped to the client/session that created it; `permId` is the broker-side persistent identifier.
- For orders originally placed by the same API client, use `orders cancel <order_id>` or the corresponding modify flow from that same client ID.
- For manual orders created in the TWS GUI, IBKR’s documented rule is to connect with **client ID 0** and bind the order before modifying it.
- If you need to cancel everything regardless of origin, use the API-level `reqGlobalCancel` escape hatch (not a normal day-to-day workflow, and not currently surfaced as a dedicated CLI command in this skill).
- `PendingCancel` means a cancel request was sent but not yet confirmed by IBKR; the order is not yet confirmed canceled and can remain in that state for a while.
- If multiple order submissions are accidentally created by retrying with different client IDs, do not assume they are the same order; verify `permId`/status before retrying again.

## Output handling

- Use `--format json` when the result needs to be post-processed or summarized programmatically.
- Use table output for quick human-readable inspection.
- For repeated polling, be explicit that the command is long-running (`account pnl --watch`, `market watch quotes ...`).
