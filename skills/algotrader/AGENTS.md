# Algotrader Agent Guide

Use repo config, not filesystem guesses.

- Config files:
  - `config/trading.defaults.toml`
  - `config/trading.local.toml`
- Primary profiles:
  - `crypto_daily`
  - `crypto_5m`
  - `equities_daily`
  - `ibkr_rotation`
- Engines:
  - NautilusTrader for production-style backtests and execution validation
  - Rust engine for parameter search, batch research, and portfolio comparisons
- Strategy location:
  - configured by `paths.strategies_dir`
- Lab notebook:
  - configured by `paths.lab_notebook_path`
- Results root:
  - configured by `artifacts.results_root`

Live watch-and-wake boundary:

- This skill and guide are for research, backtests, and parameter-search workflows.
- For live stock alerting, use the sibling `trading-tools` runtime instead of treating algotrader as the alert engine.
- The current stock watch families there are:
  - `equity_level_watch`
  - `equity_vwap_bounce_watch`
  - `equity_vwap_reclaim_watch`
- The current live source path there is TWS closed 1-minute bars feeding `trade-daemon`, with `assistant-bot` handling downstream Discord wake delivery.
- If the ask is "wake the AI at a level or on VWAP behavior", do not invent a new autonomous strategy or backtest workflow first.

Do not pass `--data-dir` or other path flags. Use `--profile` where a dataset selection is needed.
