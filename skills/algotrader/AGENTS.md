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

Do not pass `--data-dir` or other path flags. Use `--profile` where a dataset selection is needed.
