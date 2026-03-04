# Volume Clock (VPIN)
Volume-Synchronized Probability of Informed Trading. Estimates informed trading activity using tick rule + equal-volume bucketing. VPIN ∈ [0,1]: 1 = all flow one-directional (maximum toxicity).

## Use Cases
- Informed-flow detection: rising VPIN precedes large directional moves
- Liquidity risk: high VPIN signals elevated adverse selection — wide spreads expected
- Regime pre-cursor: VPIN spike often leads price breakout by 1–3 bars

## Limitations
- Requires volume data; pure price-only feeds are unsupported
- Tick rule (close[t] > close[t-1] = buy) is a coarse direction classifier — misclassifies at turning points
- Sensitive to `n_buckets`: too few buckets → noisy; too many → slow to update
- Not exchange-flow: uses bar-level volume, not true tape data

## Algorithm

1. Divide total window volume into `n_buckets` equal-volume buckets
2. Classify each bar's volume as buy (close rises) or sell (close falls)
3. Fill buckets sequentially; a single bar may span multiple buckets
4. For each completed bucket: `imbalance = |buy_vol - sell_vol| / (buy_vol + sell_vol)`
5. VPIN = mean imbalance over the most-recent `n_buckets` completed buckets

## Parameters

| Parameter | Default | Description |
|---|---|---|
| `period` | — | Rolling window of (close, volume) observations retained |
| `n_buckets` | 50 | Number of equal-volume buckets used to compute VPIN |

## Output
- `value: f64` — VPIN ∈ [0, 1]

## Status
Implemented. Engine: `vpin.rs`. NT: `VolumeClock`.
