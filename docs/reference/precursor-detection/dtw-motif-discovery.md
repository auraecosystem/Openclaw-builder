# DTW Motif Discovery with MDL Scoring

Finds recurring patterns under time-warping using Dynamic Time Warping distance, then scores
motifs by Minimum Description Length to penalize overfitting. Captures "same pattern at different
speeds" — relevant when precursor timing varies across instances.

## Use Cases

- Finding precursor patterns that occur at varying speeds
- MDL-principled motif selection (avoids overfitting to noise)
- Complement to Matrix Profile: warping-invariant vs rigid matching
- Template-bank construction for pattern matching

## Limitations

- DTW is O(n·r) per pair with Sakoe-Chiba band — expensive at scale
- MDL coding choices affect which patterns are selected
- Time warping can align unrelated dynamics in fat-tailed data
- Z-normalization is critical (without it, amplitude dominates)

## Algorithm

**DTW:**

```
R[i,j] = d(q_i, c_j)² + min(R[i-1,j], R[i,j-1], R[i-1,j-1])
DTW(Q,C) = √R[n,m]
```

**Sakoe-Chiba band:** `|i - j·(n/m)| ≤ r`

**MDL scoring:**

```
MDL(M,D) = L(M) + L(D|M)
L(M) = l · log₂(a)
L(D|M) = k·log₂(n) + (n/2)·log₂(2πe·σ_r²)
```

Best motif = argmin MDL.

## Parameters

| Parameter | Symbol | Default | Range | Description |
|---|---|---|---|---|
| Window | `period` | 500 | 100–2000 | Search window |
| Subseq range | `subseq_min/max` | 16/128 | 8–512 | Motif length range |
| Warping radius | `warping_radius` | 0.1×len | 0.05–0.2×len | Band width |
| Z-normalize | `z_normalize` | true | — | Per-subsequence |
| SAX alphabet | `alphabet_size` | 4 | 3–8 | For MDL encoding |
| Top-k | `top_k` | 3 | 1–10 | Motifs to track |

## Complexity

DTW per pair: O(n·r). Full search: O(n²·r). Prunable with LB_Keogh.

## Output

- `value: f64` — MDL score of best motif
- `best_motif_length: usize`, `n_occurrences: usize`

## References

Sakoe & Chiba (1978), Mueen et al. (2009), Rakthanmanon et al. (2012)

## Status

To implement. Zero deps. Tier 4 (specialized).
