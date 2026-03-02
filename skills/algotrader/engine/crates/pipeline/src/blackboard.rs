//! Named intermediate storage for pipeline execution.
//!
//! Each pipeline step writes its output `Slot` to the `Blackboard` by ID.
//! The next step reads the previous output implicitly (via `last()`), or any
//! earlier step by explicit ID lookup. This decouples step execution order
//! from data dependencies.

use std::collections::HashMap;

use engine_types::{SignalSet, WideMask, WideMatrix};

/// Tagged union of pipeline intermediate results.
#[derive(Clone)]
pub enum Slot {
    Mask(WideMask),
    Matrix(WideMatrix),
    Signals(SignalSet),
    Scalar(Vec<f32>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotType {
    Mask,
    Matrix,
    Signals,
    Scalar,
}

impl Slot {
    pub fn slot_type(&self) -> SlotType {
        match self {
            Self::Mask(_) => SlotType::Mask,
            Self::Matrix(_) => SlotType::Matrix,
            Self::Signals(_) => SlotType::Signals,
            Self::Scalar(_) => SlotType::Scalar,
        }
    }

    pub fn as_mask(&self) -> Option<&WideMask> {
        if let Self::Mask(m) = self {
            Some(m)
        } else {
            None
        }
    }

    pub fn as_matrix(&self) -> Option<&WideMatrix> {
        if let Self::Matrix(m) = self {
            Some(m)
        } else {
            None
        }
    }

    pub fn as_signals(&self) -> Option<&SignalSet> {
        if let Self::Signals(s) = self {
            Some(s)
        } else {
            None
        }
    }

    pub fn as_scalar(&self) -> Option<&Vec<f32>> {
        if let Self::Scalar(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

/// Ordered key-value store for pipeline intermediate results.
///
/// Tracks insertion order so `last()` returns the most recently written slot,
/// enabling implicit chaining between consecutive pipeline steps.
pub struct Blackboard {
    slots: HashMap<String, Slot>,
    order: Vec<String>,
}

impl Blackboard {
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn put(&mut self, id: String, slot: Slot) {
        self.slots.insert(id.clone(), slot);
        self.order.push(id);
    }

    pub fn get(&self, id: &str) -> Option<&Slot> {
        self.slots.get(id)
    }

    /// Returns the most recently inserted slot.
    pub fn last(&self) -> Option<&Slot> {
        self.order.last().and_then(|id| self.slots.get(id))
    }

    /// Returns the ID of the most recently inserted slot.
    pub fn last_id(&self) -> Option<&str> {
        self.order.last().map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_types::{WideMask, WideMatrix};

    #[test]
    fn put_and_get() {
        let mut bb = Blackboard::new();
        let mask = WideMask::new_false(2, 3);
        bb.put("step_1".into(), Slot::Mask(mask));

        assert!(bb.get("step_1").is_some());
        assert!(bb.get("step_1").unwrap().as_mask().is_some());
        assert!(bb.get("nonexistent").is_none());
    }

    #[test]
    fn last_tracking() {
        let mut bb = Blackboard::new();
        assert!(bb.last().is_none());
        assert!(bb.last_id().is_none());

        bb.put("a".into(), Slot::Scalar(vec![1.0, 2.0]));
        assert_eq!(bb.last_id(), Some("a"));
        assert!(bb.last().unwrap().as_scalar().is_some());

        let mat = WideMatrix::new(vec![0.0; 6], 2, 3);
        bb.put("b".into(), Slot::Matrix(mat));
        assert_eq!(bb.last_id(), Some("b"));
        assert!(bb.last().unwrap().as_matrix().is_some());
    }

    #[test]
    fn type_accessors() {
        let mask_slot = Slot::Mask(WideMask::new_false(1, 1));
        assert_eq!(mask_slot.slot_type(), SlotType::Mask);
        assert!(mask_slot.as_mask().is_some());
        assert!(mask_slot.as_matrix().is_none());
        assert!(mask_slot.as_signals().is_none());
        assert!(mask_slot.as_scalar().is_none());

        let scalar_slot = Slot::Scalar(vec![3.14]);
        assert_eq!(scalar_slot.slot_type(), SlotType::Scalar);
        assert!(scalar_slot.as_scalar().is_some());
        assert!(scalar_slot.as_mask().is_none());
    }
}
