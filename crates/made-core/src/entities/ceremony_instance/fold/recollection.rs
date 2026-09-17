use crate::entities::ceremony_events::MemoryRecalled;
use crate::entities::CeremonyInstance;

impl CeremonyInstance {
    /// What this session was told when it opened.
    ///
    /// Written once, at the opening, and never again: recalling is
    /// something a session does when it starts, and a second one would
    /// mean the session had been opened twice.
    pub(super) fn apply_memory_recalled(&mut self, recalled: &MemoryRecalled) {
        self.updated_at = recalled.recalled_at;
        self.recollection = Some(recalled.recollection.clone());
    }
}
