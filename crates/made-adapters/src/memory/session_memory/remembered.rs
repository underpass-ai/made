use std::collections::BTreeSet;

use made_core::value_objects::{MemoryEntry, MemoryRelation};

#[derive(Debug, Default)]
pub(super) struct Remembered {
    pub(super) entries: Vec<MemoryEntry>,
    pub(super) relations: Vec<MemoryRelation>,
    pub(super) keys: BTreeSet<String>,
}
