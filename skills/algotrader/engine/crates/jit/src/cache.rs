//! Compiled filter cache keyed by active condition signature.
//!
//! When CMA-ES evolves pipeline structure (toggling blocks on/off), different
//! genomes may produce different subsets of active conditions. Rather than
//! recompiling for each genome, we cache compiled functions by their condition
//! signature — the sorted set of (indicator, op) pairs.

use std::collections::HashMap;
use std::sync::Arc;

use engine_data::Indicator;

use crate::compiler::{compile_filter, CmpOp, JitCondition, JitFilter};

/// Hash key for a compiled JIT filter: the ordered set of active conditions.
///
/// Two genomes with the same active blocks (same indicators, same ops, same
/// order) share a compiled function. Thresholds are NOT part of the key —
/// they're passed as runtime arguments.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct BlockSignature {
    conditions: Vec<(Indicator, CmpOp)>,
}

impl BlockSignature {
    pub fn from_conditions(conds: &[JitCondition]) -> Self {
        let conditions = conds.iter().map(|c| (c.indicator, c.op)).collect();
        Self { conditions }
    }
}

/// Cache of compiled JIT filter functions.
///
/// Thread-safe via `Arc<JitFilter>` — compiled code is immutable after
/// finalization and safe to share across rayon worker threads.
pub struct JitCache {
    cache: HashMap<BlockSignature, Arc<JitFilter>>,
    pub hits: usize,
    pub misses: usize,
}

impl JitCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// Get a cached filter or compile a new one for the given conditions.
    pub fn get_or_compile(&mut self, conditions: &[JitCondition]) -> Arc<JitFilter> {
        let sig = BlockSignature::from_conditions(conditions);
        if let Some(f) = self.cache.get(&sig) {
            self.hits += 1;
            return f.clone();
        }
        self.misses += 1;
        let filter = Arc::new(compile_filter(conditions));
        self.cache.insert(sig, filter.clone());
        filter
    }

    pub fn unique_entries(&self) -> usize {
        self.cache.len()
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.hits = 0;
        self.misses = 0;
    }
}
