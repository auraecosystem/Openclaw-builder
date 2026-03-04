# Nautilus Strategy: Short-Horizon Crypto Momentum Parameters

## Summary

Added 7 configurable parameters to both `QullamaggieConfig` and `PrecomputedConfig` for short-horizon crypto momentum trading, and updated all strategy logic to use them instead of hardcoded values.

## Changes Made

**File:** `/Users/ad/work/ai/openclaw/skills/swingtrader/nautilus/strategy.py`

### New Config Fields (both QullamaggieConfig and PrecomputedConfig)

- `split_frac` (float, default 0.50) -- partial exit fraction, replacing hardcoded 50% divide-by-2
- `max_hold_bars` (int, default 0) -- time stop that closes after N bars (0 = disabled)
- `flag_min_pole` (float, default 0.20) -- minimum pole move for flag pattern
- `flag_max_retrace` (float, default 0.50) -- maximum retrace for flag pattern
- `flag_min_days` (int, default 5) -- minimum flag duration
- `flag_max_days` (int, default 25) -- maximum flag duration
- `rs_lookback` (int, default 63) -- configurable return period for RS ranking

### Logic Changes

1. **QullamaggieBreakout.__init__**: Added `self.ret_rs` indicator dict for configurable RS lookback.
2. **QullamaggieBreakout.on_start**: Creates `RollingReturn(config.rs_lookback)`, stores in `self.ret_rs`, registers with bar type.
3. **Both _manage_position methods**: Replaced `half = int(qty / 2)` with `sell_qty = int(qty * self.config.split_frac)`. Added time stop check before trailing SMA exit.
4. **QullamaggieBreakout._evaluate_entry**: RS ranking now uses `self.ret_rs` instead of `self.ret_63`. Flag pattern thresholds reference config fields.
5. **PrecomputedBreakout._evaluate_entry**: RS ranking uses dynamic `f"ret_{self.config.rs_lookback}"` key. Flag pattern thresholds reference config fields.
6. **QullamaggieBreakout.on_reset**: Added `self.ret_rs` to the reset tuple.

### Defaults preserve existing behavior

All new parameters default to values matching the previously hardcoded constants, so existing configs and backtests produce identical results without changes.

## Status

All 9 edit sections from the instructions applied successfully. File grew from 798 to 829 lines.