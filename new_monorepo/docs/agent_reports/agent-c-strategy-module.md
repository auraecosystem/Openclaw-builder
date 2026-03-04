# Agent C: strategy/ module

## Task
Write the `strategy/` module (4 files) for the Rust backtest engine refactor, porting filter and signal logic from the monolithic `filter.rs` and `signal.rs` into trait-based Setup implementations.

## Files Written

1. **`src/strategy/mod.rs`** (71 lines) -- Setup trait definition with 7 methods (name, direction, equity_fraction, fill_mode, exit_rules, filter, signals) plus the `create_setups()` factory function. Re-exports all concrete types.

2. **`src/strategy/breakout.rs`** (354 lines) -- Two structs sharing private helpers:
   - `compute_regime()`: EMA10 vs EMA20 benchmark filter (ported from `filter.rs`)
   - `breakout_filter()`: fused single-pass universe filter with RS, 52w-high, prior-move, extension, ADR%, consolidation checks
   - `breakout_signals()`: VCP/flag pattern detection, breakout confirmation, volume spike, stop calculation
   - `BreakoutQuick`: equity_fraction=0.5, exits=[ProfitTarget{5}, SmaCross{Sma10}, StopLoss]
   - `BreakoutRunner`: equity_fraction=0.5, exits=[BreakevenUpgrade{5}, SmaCross{Sma10}, StopLoss]

3. **`src/strategy/ep.rs`** (191 lines) -- EpisodicPivot struct:
   - `ep_filter()`: gap-up >= 10%, volume spike 2x, $100M dollar volume, prior neglect < 30% 6M return
   - `ep_signals()`: entry shifted to day after gap (lookahead-safe), stop = gap day low
   - fill_mode = SameDayOpen, exits=[SmaCross{Sma10}, StopLoss]

4. **`src/strategy/parabolic.rs`** (194 lines) -- ParabolicShort struct:
   - `parabolic_filter()`: parabolic run (50%/300% in 10d), first red day after 3+ green
   - `parabolic_signals()`: entry on filter day, stop = entry day high, cover at SMA10/SMA20
   - direction = Short, exits=[SmaCover{Sma10, Sma20}, StopLoss]

## API Migration Applied

All DataStore field accesses updated from old god-struct style to new accessor pattern:
- `store.close.get(r,c)` -> `store.close().get(r,c)` (convenience methods for OHLCV)
- `store.sma_10.get(r,c)` -> `store.get(Indicator::Sma10).get(r,c)` (indicator enum)
- `WideMatrix { data, n_rows, n_cols }` -> `WideMatrix::new(data, n_rows, n_cols)` (private fields)
- `store.axes.n_rows` remains direct field access (public)

## Design Decisions

- **Shared breakout helpers are module-private functions**, not methods on a shared base struct. This avoids inheritance complexity while keeping DRY -- both BreakoutQuick and BreakoutRunner delegate to the same `breakout_filter()` and `breakout_signals()`.
- **Matrix references cached at function top** (e.g. `let close_m = store.close()`) to avoid repeated accessor calls in hot loops.
- **WideMask `.data[i]` direct access** used for indexed writes in signal functions. This is valid because `data` is `pub(crate)` and all strategy code is within the crate.
- **equity_fraction returns 0.5** for both breakout halves. The actual `split_frac` from Params is applied by the caller (`lib.rs`) at simulation time, keeping the Setup trait params-free for this method.

## Status
Completed as specified. All logic faithfully ported from the original `filter.rs` and `signal.rs` with the new DataStore API.
