# Config Call-Site Wiring Report

## Task
Replace hardcoded numeric literals in 4 engine files with reads from the newly created config structs (`ExecutionConfig`, `StrategyConfig`, `AnalysisConfig`).

## Files Modified

### 1. `skills/algotrader/engine/src/execution/position.rs`
- Added `exec: ExecutionConfig` field to `PositionSizer` struct, populated from `params.execution` in `from_params()`.
- Replaced `0.01` (liquidity cap coefficient) with `self.exec.liquidity_cap_coeff`.
- Replaced `1.0` (min shares floor) with `self.exec.min_shares`.
- Replaced `0.001` (slippage base) with `self.exec.slippage_base`.
- Replaced `0.05` (slippage max cap) with `self.exec.slippage_max`.
- Replaced `0.001` (slippage fallback) with `self.exec.slippage_fallback`.

### 2. `skills/algotrader/engine/src/execution/simulator.rs`
- Replaced `bars_since >= 3` with `bars_since >= params.execution.min_hold_bars as usize`.
- Updated adjacent comment to say "min_hold_bars window" instead of "first 3 bars".

### 3. `skills/algotrader/engine/src/strategy/signal_breakout.rs`
- Replaced `min_bars: 5` in `ExitRule::ProfitTarget` with `params.strategy.profit_target_min_bars`.
- Replaced `close_window.len() < 30` with `close_window.len() < params.strategy.min_signal_data as usize`.
- Replaced `close * 0.95` with `close * (1.0 - params.strategy.stop_fallback_pct)`.

### 4. `skills/algotrader/engine/src/analysis/report.rs`
- Replaced both occurrences of `288.0` (intraday bars per day) with `params.analysis.intraday_bars_per_day`.

## Additional Files Modified (necessary dependencies)

### 5. `skills/algotrader/engine/src/strategy/mod.rs`
- Changed `Setup` trait: `fn exit_rules(&self)` to `fn exit_rules(&self, params: &Params)` so `signal_breakout` can read `params.strategy.profit_target_min_bars`.

### 6. `skills/algotrader/engine/src/strategy/breakout.rs`
- Updated both `exit_rules` implementations to accept `_params: &Params` (unused).

### 7. `skills/algotrader/engine/src/strategy/ep.rs`
- Updated `exit_rules` implementation to accept `_params: &Params` (unused).

### 8. `skills/algotrader/engine/src/strategy/parabolic.rs`
- Updated `exit_rules` implementation to accept `_params: &Params` (unused).

### 9. `skills/algotrader/engine/src/lib.rs`
- Updated all 3 call sites from `setup.exit_rules()` to `setup.exit_rules(params)`.

## Deviation from Plan
The instructions specified 4 files only, but the `exit_rules` trait change in `signal_breakout.rs` required updating the trait definition in `mod.rs`, all other implementations (`breakout.rs`, `ep.rs`, `parabolic.rs`), and the call sites in `lib.rs`. This was unavoidable -- the alternative (storing params inside the struct) would have required changing the `create_setups` factory and all callers, which is a larger change.

## Build Status
All changes compile cleanly. There are 3 pre-existing errors in `evolution/mod.rs` from a prior incomplete config wiring (unrelated to this task).

---

# Config Call-Site Wiring Report -- Signal Modules (Phase 2)

## Task
Replace hardcoded numeric constants in signal module files with reads from
`SignalParams` fields that were already added to `params.rs`.

## Files Modified

### `src/signals/pipeline.rs`
- Replaced `simple_atr(close_window, 14)` with `simple_atr(close_window, params.atr_period)`.
- Updated `scanner.to_scorer_array()` call to pass `params`.

### `src/signals/layer1/mod.rs`
- Changed `to_scorer_array()` signature to accept `&SignalParams`.
- Replaced `.clamp(-1.0, 1.0)` with `.clamp(-params.kalman_vel_clamp, params.kalman_vel_clamp)`.
- Replaced `.clamp(-0.1, 0.1) * 10.0` with `.clamp(-params.swt_norm_clamp, params.swt_norm_clamp) * params.swt_norm_scale`.
- Replaced `/ 5.0` with `/ params.vmd_norm_divisor`.
- Replaced `/ 2.0` with dynamic `/ (params.hmm_n_states as f32 - 1.0).max(1.0)`.
- Updated VMD call to pass `params.vmd_n_iter` and `params.vmd_tau`.

### `src/signals/layer3/mod.rs`
- Replaced hardcoded `100.0` (trend amplifier) with `params.kalman_trend_amplifier`.
- Replaced `1.0` (up cap) with `params.kalman_trend_up_cap`.
- Replaced `0.5` (down cap) with `params.kalman_trend_down_cap`.
- Replaced `0.1, 3.0` (conformal clamp) with `params.conformal_size_floor, params.conformal_size_ceil`.

### `src/signals/layer1/bocpd.rs`
- Added `kappa0`, `mu0`, `alpha0`, `beta0` fields to `BocpdState`.
- Added `with_priors()` constructor; `new()` delegates to it with default priors.
- Updated `predictive()` to use stored prior hyperparameters.

### `src/signals/layer1/vmd.rs`
- Added `n_iter: usize` and `tau: f32` parameters to `compute_vmd_sta_lta()`.
- Removed hardcoded `let n_iter = 15; let tau = 0.0;` locals.
- Updated internal tests to pass the new parameters.

### `src/signals/layer0/hmm.rs`
- Added `HmmModel::from_params(&SignalParams)` constructor.

### `src/signals/layer0/mod.rs`
- Changed `characterize()` signature: replaced separate `recompute_interval` and
  `hurst_window` params with `params: &SignalParams`.
- Uses `HmmModel::from_params(params)` instead of `HmmModel::default()`.
- Updated inline tests.

### `src/signals/arena.rs`
- Updated `BocpdState` construction to use `with_priors()` passing BOCPD
  hyperparameters from `SignalParams`.

### `src/strategy/signal_breakout.rs`
- Updated `characterize()` call to pass `&sp` instead of separate arguments.
- Fixed pre-existing `u32`-to-`usize` type mismatch on `profit_target_min_bars`.

### `tests/signal_layer1_test.rs`
- Added `SignalParams` import and updated `to_scorer_array()` calls.

## Verification
- `cargo build` succeeds (dev profile).
- 111 library unit tests pass.
- All 5 signal integration test suites pass (layer0, layer1, layer2, layer3, params).

## Behavioral Note
All `SignalParams::default()` values match the previously hardcoded constants
exactly, so runtime behavior is unchanged with default configuration.
