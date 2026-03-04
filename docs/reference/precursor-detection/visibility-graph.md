# Visibility Graph
Converts time series into a graph where edges represent mutual visibility between data points. Regime changes appear as changes in graph topology — degree distribution shifts from exponential (random) to power-law (fractal/trending).

## Use Cases
- Detecting fractal structure in price series (power-law degree → long memory)
- Regime classification via topological metrics (clustering, assortativity)
- Irreversibility analysis with directed visibility (trending vs mean-reverting)
- Complement to Hurst exponent: geometric rather than statistical view

## Limitations
- Sensitive to preprocessing (detrending/differencing)
- Heavy tails can dominate graph statistics
- Interpretability depends on stable mapping from dynamics to topology
- NVG is O(N²) naive, O(N log N) optimized

## Algorithm

**Natural Visibility Graph (NVG):** nodes i,j (i<j) connected iff for ALL k with i<k<j:
```
y_k < y_j + (y_i - y_j) · (j - k) / (j - i)
```

**Horizontal Visibility Graph (HVG):** nodes i,j connected iff for ALL k:
```
y_k < min(y_i, y_j)
```

**Metrics:** mean degree, degree distribution P(k), clustering coefficient, assortativity, average path length.

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Window | `period` | 100 | 30–500 | Sliding window |
| Graph type | `graph_type` | nvg | nvg/hvg | Visibility rule |
| Directed | `directed` | false | — | Time-directed edges |
| Metric | `metric` | mean_degree | degree/clustering/assortativity | Primary output |

## Complexity
NVG: O(N²) naive, O(N log N) divide-and-conquer. HVG: O(N).

## Output
- `value: f64` — selected metric
- `mean_degree: f64`, `clustering: f64`, `assortativity: f64`

## References
Lacasa et al. (2008) PNAS 105:4972, Luque et al. (2009) Phys Rev E 80:046103

## Status
To implement. Zero deps. Tier 3.
