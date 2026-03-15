# Algotrader Commands

Config-first commands:

- Build crypto daily data:
  - `uv run scripts/crypto_download.py --profile crypto_daily`
  - `uv run scripts/crypto_build.py --profile crypto_daily`
- Build crypto 5m data:
  - `uv run scripts/crypto_build_5m.py --profile crypto_5m`
- Rust engine backtest:
  - `cargo run --release --bin algotrader-engine -- --profile equities_daily --setup breakout`
- Rust evolution helper:
  - `python3 scripts/evolve.py --profile equities_daily --generations 50 --pop-size 100`
- Nautilus crypto validation:
  - `python -m nautilus.validate --profile crypto_5m --resample 1h`
- Nautilus IBKR backtest:
  - `python -m nautilus.run_backtest_ibkr --profile ibkr_rotation`

Path roots come from `config/trading.local.toml`.
