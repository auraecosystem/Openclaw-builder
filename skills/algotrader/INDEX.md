# Experiment Index

Canonical data: `lab-notebook.jsonl` (append-only, one JSON object per line).

---

## EXP-001 — Crypto Baseline: Adapting Stock Filters for 100-Ticker Universe

**Date**: 2026-02-26 | **Status**: Promoted to robustness | **Engine**: Rust (historical)

Stock-tuned defaults (rs_pct=0.02, min_adv=150M) produce zero trades on the 100-ticker crypto universe. Relaxing filters to crypto-appropriate ranges (rs_pct=0.40, no ADV/price/vol gates, wider consolidation tolerances) yields 87 trades with PF 4.49, Sharpe 1.51, +44% return over the 2017–2021 training set, 4.6% max drawdown. RS percentile sweep (0.10–0.80) shows a smooth response surface — no fragile peak. Runner half generates 2x the PnL of the quick half despite lower win rate, consistent with momentum tail capture.

| Metric | Value |
|--------|-------|
| Trades | 87 |
| Win rate | 54% |
| Profit factor | 4.49 |
| Sharpe | 1.51 |
| Return | +44.4% |
| Max DD | 4.6% |

**Note**: Rust engine results are historical reference only. See EXP-002b.

---

## EXP-002 / EXP-002b — Cross-Engine Validation: Rust vs NautilusTrader

**Date**: 2026-02-26 | **Status**: Decision made (EXP-002b corrects EXP-002)

Same crypto-adapted params run through both engines on 2017–2021 training data. Both profitable, confirming the core breakout logic works. NautilusTrader produces more trades (208 vs 87) at lower selectivity (34% WR, PF 1.44) because the Rust engine applies additional hardcoded heuristic filters.

| Metric | Rust | NautilusTrader |
|--------|------|----------------|
| Instruments | 100 | 80 |
| Trades | 87 | 208 |
| Win rate | 54% | 34% |
| PF | 4.49 | 1.44 |
| Return | +44.4% | +28.1% |

**Decision**: Adopt NautilusTrader exclusively. The Rust engine's higher PF/WR reflects opaque heuristic filters that are hard to validate and may mask overfitting. NautilusTrader provides proper event-driven execution, realistic NETTING account semantics, full order lifecycle, and a well-tested open-source matching engine. Rust engine build artifacts deleted; source retained as reference. All future experiments use NautilusTrader.

---

## EXP-003 — Short-Horizon Crypto Momentum Parameters

**Date**: 2026-02-26 | **Status**: Promoted to OOS validation | **Engine**: NautilusTrader

Added 7 new configurable parameters (split_frac, max_hold_bars, flag thresholds, rs_lookback) and swept each individually against the EXP-002 baseline (208 trades, +28.1% return). Short-horizon RS lookback and higher risk sizing are the strongest levers.

| Sweep | Best Value | Trades | Return | vs Baseline |
|-------|-----------|--------|--------|-------------|
| risk_pct | 0.030 | 209 | +93.1% | +3.3x |
| split_frac | 0.00 | 206 | +46.6% | +1.7x |
| max_hold_bars | 0 (disabled) | 208 | +28.1% | baseline |
| rs_lookback | 7 | 250 | +60.9% | +2.2x |

**Key findings**: (1) risk_pct is the strongest single lever. (2) split_frac=0.0 (no partial exit, let runners run) beats baseline by 66%. (3) Time stop hurts at all values — daily momentum needs time. (4) rs_lookback=7-21 all ~2x baseline. Best combined config: risk_pct=0.02, split_frac=0.50, max_hold_bars=10, rs_lookback=21 → **+81.1% return** (2.9x baseline).

**Note**: All in-sample. Must validate OOS before adopting.

---

## EXP-004 — 5-Minute Intraday Breakout: Multi-Timeframe Scaling

**Date**: 2026-02-26 | **Status**: Completed (negative result) | **Engine**: NautilusTrader

Added `bars_per_day` multiplier and configurable `high_lookback` to support sub-daily bar resolutions. All indicator periods (SMA, ATR, volume SMA, rolling returns, regime EMAs) scale by `bars_per_day` while pattern detectors (VCP, Flag) keep fixed 60-bar lookbacks due to O(lookback^2) complexity.

**Infrastructure changes**: `bars_per_day` and `high_lookback` added to both strategy configs, evolve.py precompute pipeline, and CLI. Pattern detector `FlagDetector` gained `max_flag_bars`/`max_pole_bars` params. Return array keys are now scaled (e.g. `ret_18144` for 63-day on 5m).

**5m results** (10 tickers, H2 2021, high_lookback=30):

| Config | Trades | Return | Win Rate | Sharpe |
|--------|--------|--------|----------|--------|
| Baseline (vol=1.2) | 1997 | -11.5% | 18.6% | 2.16 |
| High vol (vol=3.0) | 615 | -4.1% | 16.6% | 1.76 |
| Tight+pos (vol=3, rs=14d) | 594 | -3.6% | 16.8% | 1.93 |
| Very tight (vol=3, rs=21d, move=0.20) | 521 | -2.9% | 16.7% | 1.66 |

**Key findings**: (1) All configs are negative return — daily breakout logic doesn't transfer to 5m. (2) Pattern detectors with 60-bar lookback (5 hours) produce mostly noise on 5m. (3) Position sizing is always capped at `max_pos_pct` because ATR(14-day equivalent) on 5m is tiny relative to price — `risk_pct` has zero effect. (4) Volume spike filter is the only meaningful selectivity lever on 5m. (5) Flag detector fires 0% of the time on 5m (no 20%+ pole moves in 5-hour windows).

**Bugs fixed**: (1) `ret_63` lookup used unscaled key — was always NaN on non-daily bars, producing zero trades. (2) `bpd` variable used before definition in `_evaluate_entry`.

**Conclusion**: The daily Qullamaggie breakout strategy requires fundamental rethinking for intraday timeframes, not just period scaling. Pattern detection, position sizing, and entry criteria all need intraday-specific logic.
