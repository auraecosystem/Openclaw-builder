# Agent A: `data/` Module Report

## Task

Refactor the monolithic `types.rs` + `data.rs` into a structured `src/data/` module with 5 files, replacing the 34-field god struct `DataStore` with an `Indicator` enum-indexed `Vec<WideMatrix>`.

## Files Written

| File | Lines | Purpose |
|------|-------|---------|
| `src/data/indicator.rs` | 80 | `Indicator` enum (26 variants, `#[repr(u8)]`, strum derives), `cache_filename()`, `is_ohlcv()` |
| `src/data/matrix.rs` | 85 | `WideMatrix` and `WideMask` with private fields, `new()` constructors, accessors, `pub(crate) data` |
| `src/data/store.rs` | 99 | `Axes`, `IntradayData`, `DataStore` with `daily: Vec<WideMatrix>` indexed by enum discriminant |
| `src/data/loader.rs` | 391 | `load_data_store()` using `Indicator::iter()`, `resolve_date_row()`, all parquet helpers |
| `src/data/mod.rs` | 9 | Module declarations and re-exports |

**Total: 664 lines** (down from ~467 lines split across `types.rs` + `data.rs`, with better separation)

## Key Changes from Original

1. **WideMatrix/WideMask encapsulation**: Fields `n_rows`/`n_cols` are now private, exposed via accessor methods. `data` is `pub(crate)` for bulk iteration in sibling modules. Added `new()` constructor with `debug_assert_eq!` on dimensions and `data_mut()` for mutable access.

2. **Indicator enum replaces 21 named fields**: The old `DataStore` had 21 named `WideMatrix` fields (`open`, `high`, ..., `consol_high`). Now `daily: Vec<WideMatrix>` is indexed by `Indicator as usize`, with `Indicator::COUNT` (26) enforced in the constructor.

3. **IntradayData replaces 7 Option fields**: `open_5m`/`high_5m`/`low_5m`/`close_5m`/`volume_5m` + `dates_5m` + `day_to_5m` collapsed into a single `Option<IntradayData>` with `matrices: Vec<WideMatrix>`, `timestamps: Vec<i64>`, `day_mapping: Vec<(usize, usize)>`.

4. **Loader uses Indicator::iter()**: Instead of a hand-maintained `Vec<(&str, &str)>` of indicator names/filenames, the loader iterates the enum, filters by `cache_filename().is_some()`, and loads in parallel via rayon. The resulting matrices are assembled in discriminant order using a `HashMap<u8, WideMatrix>`.

5. **All parquet helpers preserved**: `load_wide_parquet`, `extract_dates`, `load_ohlcv`, `load_etf_set`, `extract_5m_timestamps`, `build_day_to_5m` are carried over verbatim as private functions in `loader.rs`, only changing `WideMatrix { data, n_rows, n_cols }` struct literals to `WideMatrix::new(data, n_rows, n_cols)`.

## Execution Notes

- Plan went exactly as specified in the refactor spec. No deviations needed.
- All API signatures match the spec targets.
- The `EnumCount` import in `store.rs` is required for the `Indicator::COUNT` assertion.
- The `IntoEnumIterator` import in `loader.rs` is required for `Indicator::iter()`.
