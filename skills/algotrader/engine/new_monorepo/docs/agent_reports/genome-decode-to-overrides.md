# Agent Report: genome decode_to_overrides / encode_from_pipeline_params

## Task

Add two new methods to `GenomeSpec` in
`/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/evolution/genome.rs`:

- `decode_to_overrides` — converts a genome vector to a `HashMap<String, serde_json::Value>`
  suitable for `DynamicSetup::from_json_with_overrides()`.
- `encode_from_pipeline_params` — initializes a genome mean from a JSON strategy's `params`
  block defaults, used to seed CMA-ES at the config's starting point.

## What Was Done

Inserted both methods at line 269, immediately after the existing `decode` method and before
`clamp`, with no changes to any existing code.

### `decode_to_overrides`

```rust
pub fn decode_to_overrides(&self, genome: &[f64]) -> HashMap<String, serde_json::Value> {
    let mut overrides = HashMap::new();
    for (i, b) in self.bounds.iter().enumerate() {
        let raw = genome[i].clamp(b.min, b.max);
        let val = if b.is_boolean {
            serde_json::Value::Bool(raw >= 0.5)
        } else if b.is_integer {
            serde_json::json!(raw.round() as i64)
        } else {
            serde_json::json!(raw)
        };
        overrides.insert(b.name.to_string(), val);
    }
    overrides
}
```

### `encode_from_pipeline_params`

```rust
pub fn encode_from_pipeline_params(
    &self,
    params: &HashMap<String, engine_pipeline::config::ParamValue>,
) -> Vec<f64> {
    self.bounds
        .iter()
        .map(|b| {
            params
                .get(b.name)
                .and_then(|pv| {
                    if b.is_boolean {
                        pv.as_bool().map(|v| if v { 1.0 } else { 0.0 })
                    } else {
                        pv.as_f64()
                    }
                })
                .unwrap_or((b.min + b.max) / 2.0)
        })
        .collect()
}
```

## Verification

- `HashMap` was already imported at line 7 (`use std::collections::HashMap`).
- `engine_pipeline::config::ParamValue` is already used in `encode_from_pipeline_params`
  at line 185 of the same file — import path confirmed correct.
- `cargo check` passes cleanly (0 errors, 0 warnings) after the edit.

## File Modified

`/Users/ad/work/ai/openclaw/skills/algotrader/engine/src/evolution/genome.rs`
