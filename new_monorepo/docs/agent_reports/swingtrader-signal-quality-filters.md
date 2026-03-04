# Agent Report: Swingtrader Signal Quality Filters

## Task
Implement 5 signal quality filters for the Qullamaggie swing trading backtester.

## Files Modified

- `/Users/ad/work/ai/openclaw/skills/swingtrader/lib/universe.py`
- `/Users/ad/work/ai/openclaw/skills/swingtrader/scripts/backtest.py`
- `/Users/ad/work/ai/openclaw/skills/swingtrader/scripts/backtest_duckdb.py`

## Changes Summary

### Fix 1: Default parameter updates (both scripts)
- `--rs-pct` default: `0.10` → `0.02` (aligns with Qullamaggie's 1-2% real usage)
- `--min-adv` default: `1_000_000` → `150_000_000` (institutional liquidity floor)

### Fix 2: Prior move filter — 3M return >= 30% (breakout only)
- Added `mask_prior_move(ret_63, min_ret=0.30)` to `lib/universe.py`
- `backtest.py`: computes mask from `all_indicators["returns"][63]`, passes to `get_breakout_universe(prior_move=...)`
- `backtest_duckdb.py`: added `ret_63 >= {min_prior_move} AS prior_move_ok` in `_BREAKOUT_SQL` signals CTE and `AND prior_move_ok` in WHERE; threaded `min_prior_move` through `_run_setup_duckdb` → `run_backtest` → argparse

### Fix 3: MA proximity filter — close <= 10% above 10-day SMA (breakout only)
- Added `mask_ma_proximity(close, sma, max_ext=0.10)` to `lib/universe.py` (uses `sma.where(sma > 0)` to avoid division by zero)
- `backtest.py`: computes mask from `close` and `all_indicators["sma_10"]`, passes to `get_breakout_universe(ma_proximity=...)`
- `backtest_duckdb.py`: added `(close - NULLIF(sma_10, 0)) / NULLIF(sma_10, 0) <= {max_sma_ext} AS near_sma` and `AND near_sma` in WHERE; threaded `max_sma_ext` through the same chain

### Fix 4: Market regime filter — SPY > 50-day SMA (breakout and EP, not parabolic)
- `backtest.py`: builds `regime_mask` (wide bool DataFrame via `np.tile`) when `args.regime` is True and SPY is present; applies to `liquid & stocks & regime_mask` for breakout and EP universe construction; parabolic uses plain `liquid & stocks`; added `.astype(bool)` on both code paths to guarantee bool dtype
- `backtest_duckdb.py`: computes `regime_dates: set[str]` after loading OHLCV; passes to `_run_setup_duckdb` which filters `entries_long` by date membership for breakout and EP; parabolic receives empty set

### Fix 5: `get_breakout_universe` signature extension
- Added `prior_move: pd.DataFrame | None = None` and `ma_proximity: pd.DataFrame | None = None` optional params with conditional `mask & prior_move` / `mask & ma_proximity` composition
- Backward-compatible: callers not passing these args are unaffected

## Adaptation Notes
- `numpy` was already imported in `lib/universe.py` — no new import needed
- `argparse.BooleanOptionalAction` used for `--regime` / `--no-regime` toggle (Python 3.9+, satisfies the `>=3.11` requirement in the script headers)
- Both `pd.DataFrame(True, ...)` paths in `backtest.py` got `.astype(bool)` per the implementation note about object dtype
- The `regime_dates` set is passed as empty `set()` (not `None`) for parabolic in `backtest_duckdb.py` so the conditional `if regime_dates:` block never fires for shorts

## Outcome
All planned changes implemented as specified. No deviations from the plan.
