# Algotrader — Developer Reference

General-purpose algorithmic trading backtester for crypto and US equities. Strategies are defined as JSON config files composed from a registry of 31 blocks — no Rust recompilation needed. CMA-ES evolves both numeric parameters and pipeline structure. Five legacy hardcoded strategies exist but the system is strategy-agnostic. Primary engine is NautilusTrader (production); Rust engine is for fast parameter search and evolutionary optimization.

---

## Architecture

```
data pipeline          backtest engines          optimization
─────────────          ────────────────          ────────────
Binance CSVs           NautilusTrader            Rust engine
IBKR bars        →     nautilus/strategy.py  →   engine/ (cargo)
  → ohlcv.parquet      (production)              (param search)
  → cache/*.parquet                               ↓
                                                  engine-pipeline
                                                  (JSON strategies)
```

- **NautilusTrader** (`nautilus/`): event-driven, NETTING semantics, Binance + IBKR adapters. Production backtesting. Currently has one hardcoded strategy (Qullamaggie breakout); not yet generalized.
- **Rust engine** (`engine/`): Cargo workspace, 6 crates. Strategy-agnostic — any strategy expressible as a JSON pipeline of composable blocks. CMA-ES evolution, filter fusion, fast batch parameter sweeps.
- **Python libs** (`lib/`): indicators, patterns, universe filters, risk sizing, cache I/O.

### How a strategy runs

1. `create_setups("name")` looks up hardcoded strategies by name, or loads `strategies/{name}.json` as a `DynamicSetup`
2. JSON config is parsed (`@param` interpolation, `"use"` expansion, step ID assignment), validated (type chain, block existence), and compiled into a `PhysicalPlan`
3. Consecutive fusable filter blocks are auto-coalesced into `FusedFilter` — single vectorized scan
4. Filter phase runs on the blackboard (seed → mask narrowing), signal phase produces entries/exits/stops
5. `DynamicSetupAdapter` bridges to the `Setup` trait — simulator sees no difference from hardcoded strategies

---

## Design Philosophy: Weights Over Hard Filters

**Avoid binary include/exclude decisions in scanners and screeners.** Hard filters (e.g. "RS rank must be > 0.7", "volume must be > 500k") are fragile — they discard potentially useful signals, create arbitrary cliff edges, and bake in assumptions that the optimizer can't challenge.

Instead, **express every criterion as a continuous weight or score**. A ticker with low RS shouldn't be excluded — it should score low, which the evolutionary algorithm, signal pipeline, or ML model will naturally deprioritize. If a criterion genuinely has no predictive power, the optimization process will drive its weight toward zero on its own. If it has some edge in certain regimes, it can be upweighted there.

The only legitimate hard filters are:
- **Data integrity** (no bars at all, zero volume, corrupt OHLCV)
- **Regulatory/structural** (exchange-delisted, Canadian-listed if trading via IBKR)

Everything else — RS rank, volume threshold, distance from 52w high, pattern quality score — belongs in the weight/score layer, not the filter layer.

This principle applies to `lib/universe.py`, `lib/signals.py`, the Rust signal scorer (`engine/crates/signals/scorer.rs`), and any future ML feature selection.

---

## Dynamic Strategy Pipeline (`engine-pipeline`)

The core of the system. Strategies are defined as JSON config files without recompilation. Three layers:

1. **Config (JSON)**: Human-readable strategy files with `@param` interpolation, `"use"` block imports, `"when"` conditionals, and evolvable param bounds.
2. **Logical Plan**: Parsed config → validated steps with resolved types and dependencies.
3. **Physical Plan**: Optimized for execution — consecutive simple filters fused into a single vectorized scan.

### Block Registry (31 blocks)

| Category | Blocks |
|---|---|
| Universe Filters (13) | `exclude_etf`, `price_floor`, `volume_floor`, `adv_floor`, `indicator_gte/lte`, `rs_percentile`, `near_52w_high`, `prior_move`, `adr_floor`, `extension_cap`, `consolidation`, `regime_ema` |
| Pattern Detectors (5) | `vcp`, `flag`, `gap_up`, `parabolic_run`, `pattern_any` |
| Entry Conditions (3) | `breakout_above`, `gap_entry`, `score_threshold` |
| Stop Generators (3) | `atr_capped_low`, `fixed_pct`, `gap_day_low` |
| Mask Combiners (3) | `and`, `or`, `not` |
| Signal Pipeline (1) | `signal_pipeline` (wraps 18-algorithm L0-L3 pipeline, stub) |
| Cross-Sectional (3) | `ising_susceptibility`, `vn_entropy`, `quorum` (stubs, return all-pass) |

### Filter Fusion

Consecutive blocks that check a single indicator against a threshold are auto-coalesced into a single `FusedFilter` — one pass over all indicator matrices with short-circuit evaluation per cell. A block is fusable when it produces a Mask, reads exactly one indicator per cell, compares against a threshold, and has no `when` conditional or explicit `input` reference.

### CMA-ES Pipeline Evolution

`GenomeSpec::from_pipeline_params()` auto-generates a genome spec from a strategy's `params` block. Numeric params with `min`/`max` bounds become genes. Bool params with `"evolvable": true` become continuous [0,1] genes thresholded at 0.5. The optimizer can evolve both numeric thresholds AND toggle pipeline steps on/off.

### Key Types

```
Slot:       Mask(WideMask) | Matrix(WideMatrix) | Signals(SignalSet) | Scalar(Vec<f32>)
Blackboard: HashMap<String, Slot> with ordered insertion (implicit chaining)
Block:      trait { name, output_type, input_type, execute(BlockContext), fusable_spec }
FusedFilter: Vec<FusedCondition> — single-pass vectorized scan
DynamicSetup: from_json/from_file → implements Setup trait via DynamicSetupAdapter
```

### Example: defining a strategy

```json
{
    "name": "my_strategy",
    "direction": "long",
    "fill_mode": "next_day_open",
    "equity_fraction": 0.5,
    "pipeline": [
        { "use": "stock_universe" },
        { "type": "rs_percentile", "min_pct": "@rs_pct" },
        { "type": "regime_ema", "when": "@regime" },
        { "type": "breakout_above", "vol_spike": "@vol_ratio" }
    ],
    "stop": { "type": "atr_capped_low", "fallback_pct": 0.05 },
    "exits": [
        { "type": "profit_target", "min_bars": 5 },
        { "type": "sma_trail", "period": 10 },
        { "type": "stop_loss" }
    ],
    "params": {
        "rs_pct": { "value": 0.80, "min": 0.50, "max": 0.99 },
        "regime": { "value": true, "evolvable": true },
        "vol_ratio": { "value": 2.0, "min": 1.2, "max": 4.0 }
    }
}
```

- `@param` values resolved from `params` block; CMA-ES mutates these
- `"use"` imports reusable block chains from `blocks.json`
- `"when"` conditionally enables/disables steps; CMA-ES can toggle structure
- Param `min`/`max` bounds define the evolution search space

---

## File Map

### `lib/` — shared Python libraries

| File | Purpose |
|---|---|
| `indicators.py` | ATR, SMA, EMA, rolling_return, 52w-high — vectorized wide DataFrames |
| `patterns.py` | VCP + flag detection (numba JIT), consolidation_range/high |
| `universe.py` | Liquidity/RS/52w filters, universe builders per setup, `load_ohlcv()` |
| `risk.py` | ATR-based position sizing, slippage model, stop/partial/breakeven exits |
| `signals.py` | Entry/exit signal generation (vectorbt-compatible) |
| `cache.py` | Parquet indicator cache — mtime-invalidated, writes float32 ZSTD parquet |

### `nautilus/` — NautilusTrader strategies

| File | Purpose |
|---|---|
| `strategy.py` | `QullamaggieBreakout` + `PrecomputedBreakout` + `QullamaggieConfig` (18 params) |
| `indicators.py` | NT custom indicators: VCPDetector, FlagDetector, RollingHigh, RollingReturn, VolumeSMA |
| `run_backtest.py` | Single backtest via NT BacktestEngine API |
| `run_backtest_ibkr.py` | Backtest on IBKR US equity data (parquet from `data-ibkr/`) |
| `run_ib_live.py` | Data-only NT node connected to IB Gateway (no execution, validates connection) |
| `evolve.py` | Multiprocess evolutionary optimizer (pre-compute arrays, reuse engine) |
| `validate.py` | Validate a param set; supports `--resample 1h` |
| `walkforward.py` | Rolling IS/OOS windows, computes Walk-Forward Efficiency |
| `signal_replay.py` | Replay Rust `--dump-trades` CSV in NT for cross-engine validation |
| `verify_deterministic.py` | Confirm PrecomputedBreakout == RuntimeBreakout strategy |

### `scripts/` — data pipeline and analysis

| File | Purpose |
|---|---|
| `crypto_download.py` | Async Binance kline downloader (httpx) — daily + 5m bars |
| `crypto_build.py` | Build `ohlcv_daily.parquet` + all indicator caches (US stocks: phase A/B/C) |
| `crypto_build_5m.py` | Build `data-crypto-5m/ohlcv.parquet` + 5m indicator caches |
| `backtest.py` | vectorbt backtest — 11k tickers, wide DataFrame |
| `backtest_duckdb.py` | DuckDB-accelerated vectorbt variant |
| `evolve.py` | Evolutionary optimizer driving the Rust engine (subprocess or TCP server) |
| `nautilus_backtest.py` | Load Binance CSV → NT BacktestEngine |
| `portfolio.py` | Aggregate backtest results → portfolio report |

### `scripts/ibkr/` — IBKR tools (PEP 723, run with `uv run`)

| File | Purpose |
|---|---|
| `daemon.py` | REST API daemon wrapping TWS API (FastAPI + uvicorn + ib_async) |
| `fetch_bars.py` | Fetch historical OHLCV bars, save to parquet/CSV |
| `fetch_universe.py` | Fetch 10 years of hourly bars for US equity universe |
| `test_scanner.py` | Test market scanners (75+ scan codes) |

### `engine/` — Rust Cargo workspace (6 crates, 440 tests)

| Crate | Path | Purpose |
|---|---|---|
| `engine-types` | `crates/types/` | Params, Trade, Direction, SignalParams, PatternParams, 6 config structs, WideMatrix/WideMask |
| `engine-data` | `crates/data/` | DataStore, polars loader, 26-variant Indicator enum, runtime compute |
| `engine-signals` | `crates/signals/` | 18 algorithms in flat `algorithms/` dir, `pipeline.rs`, `scorer.rs`, `execution_signals.rs`, `arena.rs` |
| `engine-patterns` | `crates/patterns/` | Fuzzy pattern scoring with NEON acceleration |
| `engine-pipeline` | `crates/pipeline/` | **Dynamic config-driven strategy pipeline** — 31 composable blocks, filter fusion, JSON configs, DynamicSetup |
| root crate | `src/` | `run_single()`, `run_batch()`, execution, strategy, analysis, evolution, portfolio, CLI, TCP server |

Key deps: polars 0.46, rayon, rustfft 6, faer 0.20, kiddo 4.
Build: `cd engine && cargo build --release` (~2.5 min full, ~1s incremental for signal-only changes).
**`engine/target/` is regenerable — delete freely to reclaim ~13GB.**

### `engine/strategies/` — JSON strategy configs

| File | Purpose |
|---|---|
| `blocks.json` | Shared block library (reusable step chains like `stock_universe`) |
| `breakout_quick.json` | Momentum breakout strategy (9 evolvable params) |
| `ep_dynamic.json` | Gap-up episodic pivot strategy (5 evolvable params) |

Unknown strategy names passed to `create_setups()` are looked up as `strategies/{name}.json` and loaded as `DynamicSetup`.

### Legacy hardcoded strategies (`engine/src/strategy/`)

Five Rust structs implementing the `Setup` trait directly: `BreakoutQuick`, `BreakoutRunner`, `EpisodicPivot`, `ParabolicShort`, `SignalBreakout`, `PatternBreakout`. These predate the dynamic pipeline and are Qullamaggie-inspired. New strategies should use JSON configs instead.

### `references/` — 20 research files

Core: `qullamaggie-rules.md`, `short-horizon-crypto-momentum.md`, `crypto-algo-trading.md`
Methods: `walk-forward-and-validation.md`, `backtesting-sins-and-biases.md`, `signal-processing-timeseries.md`
Signal processing: `signal-processing-cryptography-overlap.md`, `cross-disciplinary-signal-analysis.md`
Risk: `risk-management-position-sizing.md`, `transaction-costs-and-slippage.md`
Meta: `who-succeeds-at-algo-trading.md`, `how-to-succeed-at-algo-trading.md`, `strategy-decay-and-lifecycle.md`

---

## Data Layout

### Source data (keep — not regenerable without re-download)

| Path | Size | Shape | Format |
|---|---|---|---|
| `data/ohlcv.parquet` | 403MB | 16145 rows x 40311 cols | Wide MultiIndex `("open","GTX")`, float32, ZSTD |
| `data-crypto/ohlcv_daily.parquet` | 4.7MB | 3090 x 501 | Wide MultiIndex `("open","BTCUSDT")`, float32, ZSTD |
| `data-crypto-5m/ohlcv.parquet` | 690MB | 889253 x 501 | Wide MultiIndex, float32, ZSTD |
| `data-crypto/universe.json` | 5.6KB | — | 100 USDT pair definitions |
| `data-crypto/raw/` | 7.5GB | 13,328 CSVs | Raw Binance monthly klines, headerless, 12 cols |

`data/ohlcv.parquet` columns: 5 fields x 8063 tickers = 40311. `data-crypto` columns: 5 fields x 100 tickers + 1 timestamp = 501.

### IBKR data (`data-ibkr/`)

| File | Contents |
|---|---|
| `daily.parquet` | SPY, AAPL, QQQ, MSFT, NVDA — 1Y daily bars |
| `nvda_5y_daily.parquet` | NVDA — 5Y daily bars |
| `qqq_spy_15y_daily.parquet` | QQQ + SPY — 15Y daily bars |
| `universe_hourly.parquet` | US equity universe — 10Y hourly bars |

### Indicator caches (regenerable — delete to save space)

Caches live in `<data_dir>/cache/`. Invalidated by `ohlcv.parquet` mtime (stored in `cache/meta.json`).

| Cache dir | Shape per file | dtype | Codec | Rebuild command |
|---|---|---|---|---|
| `data/cache/*.parquet` (24 files) | 16145 x 8063 | float32 | ZSTD | `uv run scripts/crypto_build.py --data-dir data-crypto --indicators` |
| `data-crypto-5m/cache/*.parquet` (22 files) | 889253 x 101 | float32 | ZSTD | `uv run scripts/crypto_build_5m.py --data-dir data-crypto --indicators` |
| `data-crypto/cache/*.parquet` | 3090 x varies | float32 | ZSTD | same as daily build |

### Known structural quirks

- `data/ohlcv.parquet` — MultiIndex col names stored as string tuples `"('open', 'GTX')"` (vectorbt convention). Cache files use plain ticker names.
- `data-crypto-5m/cache/*.parquet` — has explicit `timestamp` col (col 0). `data/cache/*.parquet` uses implicit row index for dates.
- `data-crypto/raw/` — Binance timestamp gotcha: older files use ms (13 digits), newer use us (16 digits). `crypto_build.py` normalizes via `ts.where(ts < 1e13, ts // 1000)`.

---

## Experiments

| ID | Description | Result | Engine |
|---|---|---|---|
| EXP-001 | Crypto baseline (stock defaults) | 0 trades → relaxed: 87 trades, PF 4.49, +44.4% | Rust |
| EXP-002b | Cross-engine validation | NT adopted exclusively; Rust = param search only | Both |
| EXP-003 | Short-horizon param sweep | Best: +81.1% (2.9x baseline). Awaits OOS. | NT |
| EXP-004 | 5m intraday breakout | Negative — daily logic doesn't scale to 5m | NT |

Full experiment narratives: `INDEX.md`. Raw data: `lab-notebook.jsonl` (append-only).

### EXP-003 key findings
- `risk_pct=0.03` strongest lever (+3.3x baseline)
- `split_frac=0` (no partial exits) +1.7x
- `rs_lookback=7-21` ~2x
- Best combined: `risk_pct=0.02, split_frac=0.50, max_hold_bars=10, rs_lookback=21`

### EXP-004 lessons (5m failure)
- VCP fires on 44% of bars at 5m (noise). Flag fires 0%.
- ATR/price ~0.2% → position sizing always hits `max_pos_pct` cap
- Needs fundamentally different intraday logic (ORB, VWAP, volume profile, or dual-timeframe)

---

## Decision Framework (7 stages)

0. Hypothesis → 1. Initial Screen (PF>1, trades>=30) → 2. Robustness (2/3 sub-periods, +/-20% stable) → 3. Optimization (Sharpe<3, sensitivity) → 4. Walk-Forward (WFE>0.50) → 5. Validation (Sharpe>0.5x training) → 6. Holdout (ONE SHOT, Sharpe>1, PF>1.3, MDD<25%)

**Hard stops**: Sharpe>3.5, trades<10, >10 tunable params, holdout touched early, WFE<0.30

Full rules: `decision-framework.md`

---

## IBKR Integration

- Library: `ib_async` (community fork of ib_insync)
- TWS Classic on port 7496 (live, account U5818759). IB Gateway on port 4002 (paper).
- IBKR Desktop does NOT support TWS API — use TWS Classic or IB Gateway
- Canadian restriction: cannot algo-trade Canadian-listed securities (CIRO DMR 3200); US markets fine
- NautilusTrader has a built-in IB adapter (`InteractiveBrokersDataClient` + `InteractiveBrokersExecClient`)
- NT streaming bars require Level 1 market data subscription ($1/mo) — historical one-shot requests work without it

### `scripts/ibkr/daemon.py` — IBKR REST API

Persistent ib_async connection with HTTP endpoints. Start with `uv run scripts/ibkr/daemon.py`.

| Method | Path | Notes |
|--------|------|-------|
| GET | `/health` | Connection status, account info |
| GET | `/positions` | Open positions (cached from TWS subscription) |
| GET | `/portfolio` | Portfolio with market values and PnL (cached) |
| GET | `/orders` | Open orders with fill status (cached) |
| GET | `/bars/{symbol}` | Historical bars — query params: `duration`, `bar_size`, `what`, `rth` |
| POST | `/scanner` | Run market scanner — JSON body: `scan_code`, `location`, `above_price`, `market_cap_above` |
| GET | `/scanner/codes` | List available scan codes |

CLI: `--port 7496` (TWS live), `--port 4002` (Gateway paper), `--http-port 8000` (default).
Swagger docs at `http://localhost:8000/docs`.

### Rate limits

- Historical bars: 60 requests/10min, 6/2sec same contract, 15sec identical request cooldown
- Scanners: max 50 results, max 10 concurrent subscriptions, US market hours only
- Scanner gotchas: use `STK.US` (not `STK.US.MAJOR`), `market_cap_above=0` (cap filter needs fundamentals sub)

---

## Cross-Engine Validation Notes

- Rust `--dump-trades` CSV date = fill date (entry_row = N+1)
- NT replay: MARGIN account, fixed init_cash, bar open fill price
- Results match within 1.6% on return; trade count differs (NETTING semantics in NT)
- Gotchas: stop-market fails on gaps, CASH account depletes equity, `size_precision=0` skips BTC/ETH

## Rust Engine Bugs Fixed

- Feature normalization: raw price-scale dominated scorer → normalize in `to_scorer_array()`
- BOCPD exits: simulator never read exits mask → fixed (3-bar min hold)
- Exit rules identical: all checked `close <= active_stop` → replaced with ProfitTarget + SmaCross + StopLoss
- CMA-ES sigma explosion: clamp 1e10 → 1e2

---

## Regenerating Everything from Scratch

```bash
# 1. Python env
uv venv && uv pip install -r requirements.txt   # or per pyproject.toml

# 2. Rust engine
cd engine && cargo build --release

# 3. Crypto daily data + indicators
uv run scripts/crypto_build.py --data-dir data-crypto

# 4. 5m data + indicators
uv run scripts/crypto_build_5m.py --data-dir data-crypto

# 5. Re-download raw CSVs (if deleted)
uv run scripts/crypto_download.py --data-dir data-crypto
```

---

## Algo Trading TL;DR

Markets are a signal extraction problem. Your backtest is a hypothesis — live trading is the experiment. Most strategies fail because they were tested on the same data that generated them.

**Finding edges**: Academic factors (momentum, mean reversion), market microstructure, structural quirks (illiquid instruments institutions can't touch). Avoid anything needing >10 parameters or showing Sharpe >3.5 in backtest (overfit).

**Validation (where 90% die)**: Split data into in-sample / validation / holdout. Walk-forward analysis > single split. Perturb params +/-20% — if returns collapse, it's fragile. Monte Carlo to check if it was lucky ordering.

**Risk > Returns**: 83% of successful algo traders rank risk management #1. Use half/quarter Kelly for sizing. Run multiple uncorrelated strategies (7 strategies at <30% correlation can halve total risk). Hard drawdown rules before going live.

**Costs kill you**: Slippage + spread + market impact. Momentum strategies suffer most (chasing moves = worse fills). Add 0.1-0.2% round-trip minimum for daily strategies.

**Edges decay**: HFT lasts days, intraday momentum 3-6 months, swing systems 6-18 months, macro/factor 1-3+ years. Monitor rolling 60-day Sharpe, pause below 0.5.

**Going live**: 30+ days paper trading first. Start at 10-25% capital. Scale up after 60-90 days matching expectations.

**What matters (in order)**: Don't lose money → Don't fool yourself → Realistic costs → Strategy durability → Returns (last).

**Benchmarks**: Sharpe 1.0 = minimum viable, 1.5-2.0 = good, >2.5 = exceptional. S&P buy-and-hold is ~0.5.

---

## References — Key Excerpts

### [`qullamaggie-rules.md`](references/qullamaggie-rules.md)
Rules for the legacy breakout/EP/parabolic setups. Entry criteria, position sizing, stop placement, partial exits, hold rules. The playbook the first hardcoded strategies were based on.

### [`backtesting-sins-and-biases.md`](references/backtesting-sins-and-biases.md)
> "90% of backtests fail in live trading. The gap between backtest performance and live performance is not bad luck — it is almost always traceable to one or more systematic errors."

The seven deadly sins: lookahead bias, survivorship bias, overfitting, ignoring costs, data snooping, curve fitting, and not accounting for market impact. Essential reading before trusting any backtest result.

### [`walk-forward-and-validation.md`](references/walk-forward-and-validation.md)
> "Once you use the test set to make any decisions (including 'this strategy passes, I'll keep it'), the test set is no longer out-of-sample."

Walk-forward methodology, WFE calculation, how to structure IS/OOS windows for this dataset. Directly informs `nautilus/walkforward.py`.

### [`risk-management-position-sizing.md`](references/risk-management-position-sizing.md)
> "Risk management is the first priority. Returns are the last."

ATR-based sizing (what `lib/risk.py` implements), Kelly criterion, half/quarter Kelly, drawdown-scaled position reduction. Hard rules: max 25% in one position, stop at 2x ATR, never average down.

### [`transaction-costs-and-slippage.md`](references/transaction-costs-and-slippage.md)
> "Many strategies that appear profitable in backtests are net losers once realistic costs are modeled."

Slippage models, bid-ask spread impact, market impact at scale, realistic cost assumptions per strategy type. Minimum 0.1-0.2% round-trip for daily momentum.

### [`strategy-decay-and-lifecycle.md`](references/strategy-decay-and-lifecycle.md)
All edges die. HFT edges last days; swing edges 6-18 months. Rolling Sharpe monitoring cadence, regime change detection, when to pause vs. kill a strategy. Includes capacity limits per strategy tier.

### [`short-horizon-crypto-momentum.md`](references/short-horizon-crypto-momentum.md)
Short-hold momentum on crypto. Universe selection, RS ranking, entry/exit mechanics, why crypto behaves differently from equities (24/7, no uptick rule, thinner order books, higher vol).

### [`crypto-algo-trading.md`](references/crypto-algo-trading.md)
Structural differences between crypto and equities that matter for backtesting: exchange fragmentation, funding rates, wash trading in volume data, exchange-specific quirks (Binance delists, rebrands). Realistic edge expectations for crypto.

### [`portfolio-of-uncorrelated-strategies.md`](references/portfolio-of-uncorrelated-strategies.md)
> "If I have a diversified set of bets that are not correlated with each other, I can dramatically reduce my risk without sacrificing my returns." — Ray Dalio

Why correlation matters more than individual Sharpe ratios. 7 uncorrelated strategies at 30% correlation can halve total risk. Strategy diversification roadmap.

### [`who-succeeds-at-algo-trading.md`](references/who-succeeds-at-algo-trading.md) / [`how-to-succeed-at-algo-trading.md`](references/how-to-succeed-at-algo-trading.md)
Renaissance Medallion: 66% gross annual returns since 1988. Their hiring policy — physics/math PhDs, not finance people. Pattern across successful quants: strong math/stats, domain knowledge second, finance background last. What separates the 10% who succeed.

### [`algo-edge-sources-alpha.md`](references/algo-edge-sources-alpha.md)
Taxonomy of where edges come from: behavioral (momentum, overreaction), structural (index rebalancing, options expiry flows), microstructure (bid-ask bounce, order flow). Which categories are accessible to retail vs. institutional-only. Why momentum is the most robust documented factor.

### [`performance-metrics-and-benchmarks.md`](references/performance-metrics-and-benchmarks.md)
Full metric reference: Sharpe, Sortino, Calmar, profit factor, win rate, expectancy, MAR. Realistic benchmarks by strategy tier. Why profit factor > Sharpe for assessing trade-level quality. How to read equity curves.

### [`signal-processing-timeseries.md`](references/signal-processing-timeseries.md)
Survey of signal processing methods applicable to noisy price timeseries, organized into 8 categories: frequency-domain (FFT, STFT), wavelet (DWT, MODWT, EWT), adaptive decomposition (EMD, VMD, SSA), filtering (Kalman, Savitzky-Golay), statistical/probabilistic (HMM, BOCPD, GP), autocorrelation, ML (autoencoders, transformers), and matrix methods (DMD, Matrix Profile, PCA). Includes a practical recommendations table for crypto momentum data. Source of the 18 algorithms in `engine-signals`.

### [`signal-processing-cryptography-overlap.md`](references/signal-processing-cryptography-overlap.md)
Maps the overlap between signal processing and cryptanalysis — both domains extract hidden structure from apparently random data. Deep overlaps: Fourier/frequency analysis (oldest codebreaking technique), HMMs (cipher breaking + side-channel attacks), autocorrelation (stream cipher correlation attacks), Kalman filter (power trace analysis), SVD (chaotic cipher breaking), neural networks (deep learning attacks on AES/DES). Shannon's two foundational papers treated noise and encryption as dual problems.

### [`cross-disciplinary-signal-analysis.md`](references/cross-disciplinary-signal-analysis.md)
100 methods from 5 fields (physics, statistics/ML, biology, mathematics, quantum physics) applied to the crypto momentum system. All five fields independently diagnosed the same 3 structural gaps. Informs the signal pipeline architecture in `engine/crates/signals/`.

### [`psychology-and-behavioral-failures.md`](references/psychology-and-behavioral-failures.md)
Why psychology matters even in systematic trading: abandoning strategies during normal drawdowns, overriding signals, recency bias in parameter choices. Checklist for staying systematic under live P&L pressure.

### [`infrastructure-and-execution.md`](references/infrastructure-and-execution.md)
Minimum viable stack (data, backtest, paper trading, live execution, monitoring). Where to put execution logic vs. signal logic. Latency tiers and what matters at each. NautilusTrader's place in this stack.

### [`cameron-rules.md`](references/cameron-rules.md) / [`strategy-comparison.md`](references/strategy-comparison.md)
Ross Cameron's intraday day-trading rules (for contrast). Strategy comparison table: different timeframes, different edge sources, different risk profiles.

---

## Next Steps (as of 2026-03-02)

- Define new strategies via JSON pipeline configs and test with CMA-ES evolution
- Implement cross-sectional blocks (Ising susceptibility, vN entropy, quorum) beyond stubs
- Wire `signal_pipeline` block to engine-signals crate (currently stub)
- OOS validation of EXP-003 best config on 2022-2026 data
- Try 1h bars (`bars_per_day=24`, pattern lookback=2.5 days)
- Dual-timeframe: daily signals for direction, 5m for entry timing
- GPU compute path for FusedFilter on large datasets (wgpu, behind feature flag)
- Explore NautilusTrader IB adapter for live US equity trading
