---
name: algotrader
description: >
  Swing-trading and alpha-research skill for OHLCV-based feature engineering,
  causal data preparation, evolutionary feature discovery, orthogonality
  testing, and backtest promotion. Use when asked about: breakout research,
  feature mining, signal extraction, SQLMesh prep layers, walk-forward
  validation, or promoting candidate feature libraries into strategy tests.
metadata: { "openclaw": { "emoji": "📈", "requires": { "bins": ["uv"] } } }
---

# algotrader

Use this skill for the full research loop, not just for running a backtest.

The canonical workflow is:

1. build a deterministic point-in-time OHLCV substrate
2. define and version candidate features
3. screen candidates cheaply for relevance, stability, and redundancy
4. keep a diverse feature archive instead of one "winner"
5. assemble feature libraries and only then run expensive backtests

The detailed workflow lives in:

- `./references/feature-research-pipeline.md`
- `./research-playbook.md`
- `./experiment-templates.md`

## When to use

- "feature engineering" / "factor discovery" / "alpha mining"
- "is this feature redundant?" / "orthogonality testing"
- "how should we prepare OHLCV in SQLMesh?"
- "what should be in gold features vs labels vs splits?"
- "how should we evolve topology and hyperparameters?"
- "what gets screened before backtests?"
- "how do we promote a feature into the accepted library?"
- "how would this breakout / EP / parabolic idea be researched?"

## Core Principle

Do not optimize for the best single feature.

Optimize for a maintained library of features whose joint value is:

- predictive relevance
- temporal stability
- robustness across folds and regimes
- low redundancy with the current library
- acceptable complexity

## Preferred Workflow

1. Start with causal, normalized, multi-horizon OHLCV core features.
2. Add cross-sectional and peer-relative features if you have a universe.
3. Add dynamic relation or lead-lag features when multi-asset structure matters.
4. Add causal wavelet or other multiscale sidecars only after the base layer is strong.
5. Use evolutionary search over typed operator graphs, not unrestricted formula soup.
6. Run cheap screening before any expensive backtest.
7. Promote only feature libraries that add incremental value over the accepted baseline.

## Hard Rules

- No full-sample denoising or decomposition before splitting.
- No centered windows in predictive features.
- No train/test leakage in normalization, selection, or feature fitting.
- No giant indicator soup as the default base layer.
- No promotion based on standalone score alone; admission is library-aware.

## Feature Family Priority

Build in this order:

1. causal normalized OHLCV core
2. cross-sectional / peer-relative features
3. dynamic graph / lead-lag features
4. causal wavelet multiscale features
5. experimental sidecars such as motifs, topology, or visual encodings

The detailed ranking and reasoning are documented in:

- `./references/feature-research-pipeline.md`
- `./references/cross-disciplinary-signal-analysis.md`

## Strategy References

Use these when the task is specifically about the trading playbooks rather than the research pipeline:

- `../stock_trading/references/qullamaggie-rules.md`
- `../stock_trading/references/qullamaggie-breakout.md`
- `../stock_trading/references/qullamaggie-episodic-pivot.md`
- `../stock_trading/references/qullamaggie-parabolic-short.md`

## Data Assumptions

Assume one row is one completed bar for `(symbol, timeframe, bar_end_ts)`.

Features at time `t` may only use information known at or before `t`.
Labels, folds, and expensive validation are separate assets, not mixed into the feature rows.
