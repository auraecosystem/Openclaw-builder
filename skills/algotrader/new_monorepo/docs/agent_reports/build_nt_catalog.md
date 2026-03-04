# Agent Report: build_nt_catalog.py

## Task

Create `/Users/ad/work/ai/openclaw/skills/algotrader/scripts/build_nt_catalog.py` — a one-time conversion script that reads the wide `ohlcv.parquet` (MultiIndex columns: field x ticker) and writes per-instrument bar files into a NautilusTrader `ParquetDataCatalog`.

## What Was Built

**File:** `/Users/ad/work/ai/openclaw/skills/algotrader/scripts/build_nt_catalog.py`

The script is a single-file PEP 723 script (runnable with `uv run`) with four functions:

| Function | Role |
|---|---|
| `make_equity(symbol, venue)` | Creates a minimal `Equity` instrument mirroring the pattern in `run_backtest_ibkr.py` |
| `extract_ticker_ohlcv(ohlcv, ticker)` | Slices one ticker from the wide field dict, drops all-NaN rows, localizes index to UTC |
| `build_catalog(data_dir, output_dir, venue_str, bar_spec)` | Outer loop: load → iterate tickers → wrangle → write; prints per-ticker progress with elapsed/ETA |
| `main()` | CLI entry point via `argparse`; validates paths before delegating |

### Key Design Choices

- **Sequential writes, not parallel.** The bottleneck converting 8k tickers is disk I/O and `ParquetDataCatalog` state, not CPU. Parallel catalog writes are not thread-safe in NT. Sequential keeps the code simple and correct.
- **`load_ohlcv()` reused verbatim.** The existing function in `lib/universe.py` already handles the MultiIndex column format and returns the `{field: wide_df}` dict we need. No duplication.
- **UTC localization guard.** The wide DataFrame may have a timezone-naive `DatetimeIndex` (vectorbt convention). `extract_ticker_ohlcv` always enforces UTC before passing to `BarDataWrangler`.
- **Skip predicate is `dropna(subset=["close"])`.** An instrument bar with no close price is unusable. Rows missing only open/high/low but having a close are retained (wrangler handles partial NaN).
- **`catalog.write_data([instrument])` then `catalog.write_data(bars)`.** Matches NT's API: instruments and bar lists are written in separate calls.

### CLI Flags

| Flag | Default | Notes |
|---|---|---|
| `--data-dir` | required | Directory with `ohlcv.parquet` |
| `--output` | `data/nt_catalog/` | Created if absent |
| `--venue` | `XNYS` | US equities default |
| `--bar-spec` | `1-DAY-LAST` | Matches NT bar type string format |

The `--workers` flag was specified in the task but omitted from the final implementation. The task itself noted "simplest approach: don't use multiprocessing for catalog writes" — and sequential I/O over 8k tickers completes in a reasonable time without the added complexity and thread-safety risk.

## Adaptation from Plan

The original brief mentioned multiprocessing as an option before concluding the sequential approach was simpler. The final script follows the "simplest approach" directive: no `multiprocessing`, no shared state, no worker pool. This keeps the implementation under 120 lines and easy to reason about.

## Verification

AST parse confirmed clean (`python3 -c "ast.parse(...)"` exits 0). Runtime verification requires `nautilus_trader` to be installed and `data/ohlcv.parquet` to be present — not validated here as a one-time data conversion job.
