# engine-data: Multi-Timeframe OHLCV Resampling

## Task

Add 30m and 1h resampled OHLCV matrices to `IntradayData` in the `engine-data` crate, computed at load time from the existing 5m data.

## Files Changed

### Created
- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/crates/data/src/resample.rs`

### Modified
- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/crates/data/src/store.rs`
- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/crates/data/src/loader.rs`
- `/Users/ad/work/ai/openclaw/skills/algotrader/engine/crates/data/src/lib.rs`

## What Was Done

### `resample.rs` (new)

Pure resampling logic, no external deps beyond `engine_types::WideMatrix`.

`resample_ohlcv(open, high, low, close, volume, factor)` groups `factor` consecutive 5m bars into one coarser bar using standard OHLCV aggregation rules:
- Open = first valid bar's open
- High = max of valid highs
- Low = min of valid lows
- Close = last valid bar's close
- Volume = sum of valid volumes

All-NaN bars (outside trading hours) are skipped within each group. If an entire group is NaN, the output row remains NaN. Output has `ceil(n_5m / factor)` rows.

`resample_timestamps(timestamps, factor)` takes every `factor`-th timestamp (start of each group).

Two unit tests cover:
1. `resample_6_to_1_aggregates_correctly` — 6 bars into 1 output row, verifies O/H/L/C/V aggregation
2. `resample_partial_group_at_end` — 7 bars with factor=6 yields 2 output rows

### `store.rs`

Extended `IntradayData` with six new fields:

```rust
pub matrices_30m: Vec<WideMatrix>,
pub timestamps_30m: Vec<i64>,
pub matrices_1h: Vec<WideMatrix>,
pub timestamps_1h: Vec<i64>,
```

Original fields (`matrices`, `timestamps`, `day_mapping`) are unchanged.

### `loader.rs`

Added `use super::resample;` import. In `load_data_store`, after the 5m matrices are loaded, two resampling calls are made before constructing `IntradayData`:

```rust
let [o30, h30, l30, c30, v30] = resample::resample_ohlcv(&o5, &h5, &l5, &c5, &v5, 6);
let ts_30m = resample::resample_timestamps(&timestamps, 6);
let [o1h, h1h, l1h, c1h, v1h] = resample::resample_ohlcv(&o5, &h5, &l5, &c5, &v5, 12);
let ts_1h = resample::resample_timestamps(&timestamps, 12);
```

`DataStore::new()` and `DataStore` itself are untouched — `IntradayData` is already stored as `Option<IntradayData>`.

### `lib.rs`

Added `pub mod resample;` and `pub use resample::resample_ohlcv;` so downstream crates can call the resampler directly without touching the data store.

## Outcome

Build: `cargo build -p engine-data` — clean (zero new warnings; one pre-existing `fp-armv8` warning from `engine-types` is unrelated).

Tests: `cargo test -p engine-data resample` — 2/2 pass.

No adaption was required; implementation matched the plan exactly.
