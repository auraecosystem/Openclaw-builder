# Feature Research Pipeline

## Purpose

This document defines the preferred workflow for OHLCV-based feature research, alpha mining, and candidate promotion. The goal is not to find one "best feature." The goal is to maintain a feature library whose members are predictive, stable, robust, and not redundant with each other.

## Architecture Shape

Use this sequence:

1. canonicalize OHLCV
2. materialize point-in-time base features
3. materialize labels and folds separately
4. define candidate features as versioned artifacts
5. generate candidates with typed operators and evolutionary search
6. screen cheaply for relevance, stability, and redundancy
7. keep a diverse archive of elite candidates
8. assemble libraries or models from accepted candidates
9. run expensive backtests only on promoted bundles
10. monitor, decay, and retire accepted features

## Stage 1: Canonical Bars

The bar layer should be deterministic and boring.

Required properties:

- fixed grain: `(symbol, timeframe, bar_end_ts)`
- normalized timestamps
- duplicate handling
- missingness made explicit
- raw-vs-research series definition kept explicit
- no modeling transforms

This layer is the substrate for every later step.

## Stage 2: Point-in-Time Base Features

Build the strongest reusable deterministic base first.

Preferred base families:

1. causal normalized multi-horizon OHLCV core
2. cross-sectional and peer-relative features
3. dynamic relation and lead-lag features
4. causal wavelet multiscale sidecars

Examples in the first family:

- multi-horizon returns and log-returns
- gap returns
- intrabar returns
- candle geometry
- realized and range volatility
- compression and expansion measures
- relative volume and dollar-volume shocks
- rolling z-scores and rolling percentiles
- volatility-normalized moves

Do not start from a giant pile of classic indicators. Recent work repeatedly shows that raw OHLCV plus disciplined transforms is a stronger default than indicator soup.

## Stage 3: Labels and Splits

Labels must be stored separately from feature rows.

Examples:

- forward returns at multiple horizons
- directional labels
- threshold labels
- volatility-scaled targets

Splits must also be explicit assets:

- walk-forward folds
- expanding windows
- purge or embargo when horizons overlap

Never fit transforms on the full history and then split afterward.

## Stage 4: Candidate Definition

Every new feature candidate should be a versioned artifact with:

- `feature_id`
- family
- input columns
- topology or formula graph
- hyperparameters
- `asof_ts` contract
- owner or experiment id

If a feature is not versioned, it is not researchable.

## Stage 5: Candidate Generation

Use a curated typed operator library rather than unrestricted math.

Search dimensions:

- topology: which operations, in what order
- hyperparameters: windows, thresholds, normalizers, filters

Good generators:

- typed expression trees
- DAG or grammar-constrained search
- evolutionary search over structure plus parameters
- seeded populations from hand-built high-value primitives

The search objective is not "maximize one score." The search objective is to produce diverse, valid, useful candidates.

## Stage 6: Cheap Screening

Most candidates should die before backtesting.

First-pass screening should check:

- coverage
- missingness
- variance or entropy
- temporal stability
- relevance to target
- redundancy versus the accepted library
- complexity

Typical admission question:

"Does this candidate add stable, non-redundant information beyond what we already keep?"

## Stage 7: Redundancy and Novelty

Redundancy should be measured on outputs, not just syntax.

Use at least:

- linear dependence
- rank dependence
- nonlinear dependence if available
- incremental lift over the accepted feature library

Novelty matters because the library becomes a correlation red sea over time. A feature that looks good alone but adds nothing when combined with the existing library is not valuable.

## Stage 8: Diverse Archive

Do not keep only the current winner.

Maintain a diverse archive across niches such as:

- feature family
- complexity band
- timescale
- regime behavior
- peer-relative vs univariate vs graph-based vs multiscale

This prevents the search from collapsing into one redundant cluster.

## Stage 9: Library Assembly and Strategy Promotion

Once candidates survive screening, assemble accepted libraries or model inputs from them.

Only promoted bundles should reach expensive validation.

Validation levels:

1. feature-level
2. library-level
3. strategy-level

Backtests belong in the outer loop. They are too slow and too noisy to be the first filter on a large candidate population.

## Stage 10: Monitoring and Retirement

Accepted features are temporary.

Track:

- decay
- redundancy drift
- regime dependence
- replacement by better siblings

Useful states:

- `candidate`
- `accepted`
- `retired`

## Recommended Build Order

1. causal normalized OHLCV core
2. cross-sectional and peer-relative features
3. dynamic relation or lead-lag features
4. causal wavelet multiscale features
5. experimental sidecars such as motifs, topology, or visual encodings

If you only have one symbol, skip the peer and graph layers. If you have a broad universe, those layers become core rather than optional.

## Hard Rules

- no full-sample denoising
- no centered smoothers in predictive features
- no decomposition before splitting
- no giant indicator soup as the default base layer
- no static relation graph treated as permanent truth
- no promotion based on standalone score alone

## What the Pipeline Is Looking For

The recurring precursor pattern is:

1. compression
2. abnormal participation
3. divergence from peers
4. propagation through relations
5. multiscale burst or expansion

That sequence is more defensible than hunting for a single magical scalar feature.
