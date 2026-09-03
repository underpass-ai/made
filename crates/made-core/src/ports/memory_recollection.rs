use crate::value_objects::{MemoryEntry, MemoryRelation};

/// Entries and relations recalled from durable memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryRecollection {
    Recalled {
        entries: Vec<MemoryEntry>,
        relations: Vec<MemoryRelation>,
    },
    Unsupported,
}

impl MemoryRecollection {
    #[must_use]
    pub fn of(entries: Vec<MemoryEntry>) -> Self {
        Self::Recalled {
            entries,
            relations: Vec::new(),
        }
    }

    #[must_use]
    pub fn nothing() -> Self {
        Self::of(Vec::new())
    }

    #[must_use]
    pub fn entries(&self) -> &[MemoryEntry] {
        match self {
            Self::Recalled { entries, .. } => entries,
            Self::Unsupported => &[],
        }
    }

    #[must_use]
    pub fn relations(&self) -> &[MemoryRelation] {
        match self {
            Self::Recalled { relations, .. } => relations,
            Self::Unsupported => &[],
        }
    }

    #[must_use]
    pub const fn is_supported(&self) -> bool {
        matches!(self, Self::Recalled { .. })
    }
}
