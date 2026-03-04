# Collective Anomaly / Discord Detection

Finds the most unusual subsequence (discord) — the one with maximum distance to its nearest
non-overlapping match. Low distance = recurring motif; high distance = novel/anomalous pattern.
Extends Matrix Profile with SAX-based search heuristics.

## Use Cases

- Identifying structurally anomalous price patterns
- Motif discovery: find recurring precursor shapes
- Complement to Matrix Profile: SAX-accelerated search
- Collective anomaly detection (pattern-level, not point-level)

## Limitations

- Subsequence length is a critical parameter
- Heteroskedasticity and drift distort distance unless z-normalized
- HOT SAX heuristics improve average case but worst case is O(n²·m)
- Defining collective boundaries can be ambiguous under drift

## Algorithm

**Discord:** `argmax_i MP[i]` (maximum Matrix Profile value).

**HOT SAX:** SAX (Symbolic Aggregate Approximation) reduces subsequences to symbols. Hash SAX
words to positions. Visit rare words first (more likely discords). Inner loop: same-bucket first
+ early abandonment.

**SAX:** PAA (piecewise aggregate, w segments) to Gaussian breakpoints to alphabet of size a.

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Window | `period` | 500 | 100–2000 | Series length |
| Subsequence length | `subsequence_len` | 30 | 10–100 | Discord/motif length |
| PAA segments | `paa_segments` | 8 | 4–16 | Dimensionality reduction |
| Alphabet size | `alphabet_size` | 4 | 3–8 | SAX symbols |
| Exclusion zone | `exclusion_zone` | subsequence_len | — | Non-self-match |
| Z-normalize | `z_normalize` | true | — | Per-subsequence |

## Complexity

Brute force: O(n²·m). HOT SAX: typically O(n·m).

## Output

- `value: f64` — max discord distance
- `discord_position: usize`, `motif_distance: f64`

## References

Keogh et al. (2005) HOT SAX, Yeh et al. (2016) Matrix Profile

## Status

Matrix Profile already implemented. HOT SAX is optional extension. Tier 3.
