//! Registry mapping string names to block implementations.
//!
//! `BlockRegistry` is the central lookup table: pipeline execution resolves
//! each step's `type` field to a concrete `Block` impl via `get()`. The
//! `default_registry()` constructor will register all built-in blocks once
//! they are implemented in Waves 1A-1C.

use std::collections::HashMap;

use crate::blocks::Block;

/// Registry mapping string names to block implementations.
pub struct BlockRegistry {
    blocks: HashMap<String, Box<dyn Block>>,
}

impl BlockRegistry {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
        }
    }

    pub fn register(&mut self, block: Box<dyn Block>) {
        self.blocks.insert(block.name().to_string(), block);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Block> {
        self.blocks.get(name).map(|b| b.as_ref())
    }

    pub fn names(&self) -> Vec<&str> {
        self.blocks.keys().map(|s| s.as_str()).collect()
    }

    /// Build the default registry with all built-in blocks.
    /// Blocks are registered as they are implemented in Waves 1A-1C.
    pub fn default_registry() -> Self {
        let r = Self::new();
        // TODO: Wave 1 — register all blocks here
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blackboard::{Slot, SlotType};
    use crate::blocks::BlockContext;
    use engine_types::WideMask;

    /// Minimal mock block for registry testing.
    struct MockBlock;

    impl Block for MockBlock {
        fn name(&self) -> &'static str {
            "mock_block"
        }

        fn output_type(&self) -> SlotType {
            SlotType::Mask
        }

        fn execute(&self, ctx: &BlockContext) -> anyhow::Result<Slot> {
            let nr = ctx.range.len();
            Ok(Slot::Mask(WideMask::new_false(nr, 1)))
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = BlockRegistry::new();
        reg.register(Box::new(MockBlock));

        assert!(reg.get("mock_block").is_some());
        assert_eq!(reg.get("mock_block").unwrap().name(), "mock_block");
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn names_lists_registered() {
        let mut reg = BlockRegistry::new();
        assert!(reg.names().is_empty());

        reg.register(Box::new(MockBlock));
        let names = reg.names();
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"mock_block"));
    }

    #[test]
    fn default_registry_creates_empty() {
        let reg = BlockRegistry::default_registry();
        // No blocks registered yet (Wave 1 TODO).
        assert!(reg.names().is_empty());
    }
}
