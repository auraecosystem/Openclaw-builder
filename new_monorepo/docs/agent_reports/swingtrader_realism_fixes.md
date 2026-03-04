# Swingtrader Realism Fixes — Agent Report

## Task
Implement three backtesting realism fixes across four files in `skills/swingtrader/`.

## Files Modified

- `/Users/ad/work/ai/openclaw/skills/swingtrader/lib/signals.py`
- `/Users/ad/work/ai/openclaw/skills/swingtrader/lib/risk.py`
- `/Users/ad/work/ai/openclaw/skills/swingtrader/scripts/backtest.py`
- `/Users/ad/work/ai/openclaw/skills/swingtrader/scripts/backtest_duckdb.py`

---

## Fix 1: Fill at Next-Day Open

**Problem**: Breakout and parabolic setups were filling at the signal bar's close, creating lookahead bias (you can't trade the close of the bar that generated the signal).

**Changes**:

### `scripts/backtest.py`
- `_run_breakout_split()`: added `open_: pd.DataFrame` param; computes `fill_price = open_.shift(-1)` and passes it as `price=fill_price` to both vectorbt calls.
- `run_setup()`: added `open_` pass-through to `_run_breakout_split()`. Changed parabolic `price_df` from `close_a` to `o[active_tickers]` — EP was already correct, both now use next-day open consistently.

### `scripts/backtest_duckdb.py`
- `_run_breakout_split()`: same `open_` param addition and `fill_price = open_.shift(-1)`.
- Breakout call site: passes `open_.loc[slc][active_tickers]`.
- EP section: already used `open_.loc[slc][active_tickers]` — no change needed beyond slippage.
- Parabolic section: added `price=open_.loc[slc][active_tickers]`.

---

## Fix 2: Volume Liquidity Filter

**Problem**: Signals could fire on stocks with tiny dollar volume, which are untradeable at realistic sizes.

**Changes**:

### `lib/signals.py`
- `continuation_breakout_signals()`: added `min_adv_dollars: float = 1_000_000` param; applies `entries = entries & (vol_sma_20 * close >= min_adv_dollars)` after existing entry logic.
- `episodic_pivot_signals()`: same param and filter applied on the entry day (after shift).
- `parabolic_short_signals()`: added `vol_sma_20: pd.DataFrame | None = None` and `min_adv_dollars` params; filter applied only when `vol_sma_20` is provided (backward-compatible default).

### `scripts/backtest.py`
- `run_setup()`: added `min_adv: float = 1_000_000` param; passes `min_adv_dollars=min_adv` to all three signal calls. Also adds `vol_sma_20=ind_data["vol_sma_20"]` to EP and parabolic calls.
- CLI: `--min-adv` argument (default `1_000_000`).
- `run_backtest()`: passes `min_adv=args.min_adv` to `run_setup()`.

### `scripts/backtest_duckdb.py`
- `_run_setup_duckdb()`: added `min_adv_dollars: float = 1_000_000` param.
- `_BREAKOUT_SQL`, `_EP_SQL`, `_PARABOLIC_SQL`: each gets `AND vol_sma_20 * close >= {min_adv_dollars}` in its WHERE clause.
- All `.format()` calls pass `min_adv_dollars=min_adv_dollars`.
- CLI: `--min-adv` argument (default `1_000_000`).

---

## Fix 3: Size-Adjusted Slippage

**Problem**: Flat 10bps slippage ignores market impact — a 10,000-share order in a stock averaging 50,000 shares/day incurs far more slippage than a 100-share order.

**Changes**:

### `lib/risk.py`
Added `size_adjusted_slippage()` function before `size_from_risk()`:
```python
def size_adjusted_slippage(
    sizes: pd.DataFrame,
    vol_sma_20: pd.DataFrame,
    entries: pd.DataFrame,
    base_slippage: float = 0.001,
    k: float = 0.1,
) -> pd.DataFrame:
    adv = vol_sma_20.reindex(index=sizes.index, columns=sizes.columns)
    impact = k * np.sqrt(sizes / adv.where(adv > 0))
    slippage = impact.where(entries, other=base_slippage).fillna(base_slippage)
    return slippage.clip(lower=base_slippage, upper=0.05)
```
Uses square-root market impact model. Non-entry bars get `base_slippage`. Capped at 5%.

### `scripts/backtest.py`
- `_run_breakout_split()`: added `vol_sma_20`, `slippage_k` params; computes `slippage_df` before the vectorbt loop; passes `slippage=slippage_df` (replaces `slippage=0.001`).
- `run_setup()`: added `slippage_k: float = 0.1` param; computes `slippage_df` before EP/parabolic vbt call; passes `slippage=slippage_df`. Also passes `vol_sma_20=ind_data["vol_sma_20"][active_tickers]` and `slippage_k=slippage_k` to `_run_breakout_split()`.
- CLI: `--slippage-k` argument (default `0.1`).
- `run_backtest()`: passes `slippage_k=args.slippage_k` to `run_setup()`.

### `scripts/backtest_duckdb.py`
- `_run_breakout_split()`: same `vol_sma_20`, `slippage_k` params and slippage computation.
- `_run_setup_duckdb()`: added `vol_sma_20: pd.DataFrame`, `slippage_k: float` params. EP and parabolic sections compute `slippage_df` before vbt call.
- `run_backtest()`: loads `vol_sma_20` from parquet cache (adds it to `_PARQUET_KEYS`), computes it fresh if cache miss, passes to `_run_setup_duckdb()`.
- CLI: `--slippage-k` argument (default `0.1`).

---

## Verification

All four files pass `ast.parse()` syntax check. `vectorbt` is not installed system-wide (scripts use `uv run` inline metadata), so full `--help` test requires `uv run`.

## Notes / Adaptations

- `parabolic_short_signals()` receives `vol_sma_20` as an optional param (`None` default) rather than required, to remain backward-compatible with any direct callers that don't pass it.
- In `backtest_duckdb.py`, `vol_sma_20` is loaded from the parquet cache alongside `atr_14` and `sma_10` — it was previously loaded only by DuckDB SQL internally. This adds a small warm-up cost but avoids recomputing it from scratch when the cache exists.
- The `_run_breakout_split()` slippage DataFrame is computed on the already-sliced active_tickers data, so `reindex()` inside `size_adjusted_slippage()` is a no-op in practice (same index/columns), which is efficient.
