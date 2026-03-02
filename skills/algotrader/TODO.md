# TODO — Algotrader

## Decision: Migrate to SQLMesh + DuckDB + Rust Extensions

**Date**: 2026-03-02 | **Status**: Decided, not started

### Why

The Rust engine's dynamic pipeline (JSON blocks) is functionally correct (EXP-010: bit-identical parity with hardcoded strategies) but has structural limitations:

- **4-25x filter overhead** vs hardcoded fused loops — acceptable for single backtests, problematic for CMA-ES (thousands of iterations)
- **No intermediate caching** — every pipeline run recomputes all filters from scratch, even when only 1-2 params changed
- **Wide-matrix data model** is fast but opaque — no ad-hoc querying, no SQL exploration during research
- **JSON block composition** is less expressive than SQL for data transforms, joins, window functions

### What we're moving to

**SQLMesh** orchestrates the DAG with automatic caching, interval-aware incremental models, and virtual dev environments. **DuckDB** executes SQL in-process with vectorized columnar processing (implicit SIMD, zero-copy Arrow). **Rust DuckDB extensions** provide custom scalar/table functions for algorithms that need manual SIMD or complex logic (VCP, flag detection, signal pipeline).

### Key benefits

1. **DAG-aware caching**: when CMA-ES mutates `rs_pct` but not `min_price`, SQLMesh skips recomputing universe filters — only reruns from the changed node downstream
2. **SQL composition**: strategies become compositions of SQL models, readable by non-Rust collaborators
3. **Shared intermediates**: `universe_filtered`, `indicators_computed`, etc. are materialized once and shared across all strategies
4. **Ad-hoc research**: `SELECT * FROM ep_filter_output WHERE ticker = 'AAPL'` instead of building CLI flags
5. **DuckDB vectorized engine**: simple filters (`close > 5.0`, `vsma > 300000`) run at comparable speed to hardcoded Rust — no custom code needed
6. **128GB RAM**: entire dataset fits in DuckDB in-memory mode (~2-4GB uncompressed). No ramdisk needed.

### Migration work

- [ ] **Data model**: convert wide-matrix `(rows x tickers)` to tall/long `(date, ticker, ohlcv)` format for DuckDB. ~130M rows for US equity.
- [ ] **Rust DuckDB extensions**: port `engine-patterns` (VCP, flag, parabolic_run) and `engine-signals` (18 algorithms) as DuckDB table/scalar functions using the [Rust extension template](https://github.com/duckdb/extension-template-rs). Zero-copy via Arrow columnar buffers.
- [ ] **SQLMesh project**: set up `models/` directory with SQL models for each pipeline stage (universe filters, indicators, pattern detectors, entry/exit signals). Each strategy = a composition of models.
- [ ] **CMA-ES integration**: evolve.py calls SQLMesh with param overrides, SQLMesh reruns only changed models. Measure iteration time vs current Rust engine.
- [ ] **NautilusTrader bridge**: SQLMesh produces signal tables; NT reads them for production backtesting/execution.
- [ ] **Deprecate engine-pipeline**: once SQLMesh strategies match Rust output (new parity test), remove JSON block system.

### What stays in Rust

- `engine-signals` algorithms (FFT, Kalman, BOCPD, etc.) — too complex for SQL, ship as DuckDB extensions
- `engine-patterns` (VCP, flag detection with NEON acceleration) — same
- Position sizing / execution simulation — may stay in Rust or move to NautilusTrader

### What becomes SQL

- All universe filters (price_floor, volume_floor, adv_floor, rs_percentile, etc.)
- Indicator computation (SMA, EMA, ATR, rolling returns) — DuckDB window functions
- Filter composition and mask logic (AND/OR/NOT of filter outputs)
- Strategy DAG definition (which filters → which signals → which exits)

### Risk: Rust extension maturity

The [DuckDB Rust extension API](https://github.com/duckdb/extension-template-rs) is experimental. Custom aggregates and window functions aren't fully supported yet. Scalar and table functions cover ~90% of use cases. If we hit a wall on window functions, we can use DuckDB's Python UDF escape hatch or keep those specific computations in a pre-processing step.

### References

- [DuckDB Arrow zero-copy integration](https://duckdb.org/2021/12/03/duck-arrow)
- [SQLMesh DuckDB integration](https://sqlmesh.readthedocs.io/en/latest/integrations/engines/duckdb/)
- [SQLMesh Python models](https://sqlmesh.readthedocs.io/en/latest/concepts/models/python_models/)
- [DuckDB Rust extension template](https://github.com/duckdb/extension-template-rs)
- EXP-010 in `INDEX.md` — parity test proving dynamic pipeline correctness

---

## Active (pre-migration)

- [ ] Rebuild RS pctrank caches (`RsPctrank1m/3m/6m`) to unblock breakout parity test
- [ ] OOS validation of EXP-003 best config on 2022-2026 data
- [ ] Wire `signal_pipeline` block to engine-signals crate (currently stub)
- [ ] Implement cross-sectional blocks (Ising susceptibility, vN entropy, quorum) beyond stubs
- [ ] Try 1h bars (`bars_per_day=24`, pattern lookback=2.5 days)
- [ ] Explore NautilusTrader IB adapter for live US equity trading
