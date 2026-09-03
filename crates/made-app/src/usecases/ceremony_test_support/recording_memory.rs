use made_core::value_objects::{MemoryScope, MemoryWrite};
use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub(in crate::usecases) struct RecordingMemory {
    pub(super) written: RwLock<Vec<(MemoryScope, MemoryWrite, String)>>,
}
