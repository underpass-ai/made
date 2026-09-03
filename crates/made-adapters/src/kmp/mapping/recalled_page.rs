use made_core::value_objects::{MemoryEntry, MemoryRelation};

/// What one page of a temporal read yielded.
///
/// Entries keep the reference they came back under, because paging through a
/// temporal read can show the same entry twice and the reference is the only
/// thing that says so.
#[derive(Debug, Default)]
pub(in crate::kmp) struct RecalledPage {
    pub(in crate::kmp) entries: Vec<(String, MemoryEntry)>,
    pub(in crate::kmp) relations: Vec<MemoryRelation>,
    pub(in crate::kmp) next_cursor: Option<String>,
    /// Entries the kernel returned that this engine cannot represent.
    pub(in crate::kmp) unreadable: usize,
}
