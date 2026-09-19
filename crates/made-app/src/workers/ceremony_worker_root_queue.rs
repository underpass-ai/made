use super::{CeremonyWorkerPriority, CeremonyWorkerScheduleRequest, CeremonyWorkerWeight};

#[derive(Debug)]
pub(super) struct CeremonyWorkerRootQueue {
    pub(super) weight: CeremonyWorkerWeight,
    pub(super) priority: CeremonyWorkerPriority,
    pub(super) deficit: u64,
    pub(super) waiting_turns: u64,
    pub(super) pending: Vec<(u64, CeremonyWorkerScheduleRequest)>,
}

impl CeremonyWorkerRootQueue {
    pub(super) const fn new(
        weight: CeremonyWorkerWeight,
        priority: CeremonyWorkerPriority,
    ) -> Self {
        Self {
            weight,
            priority,
            deficit: 0,
            waiting_turns: 0,
            pending: Vec::new(),
        }
    }

    pub(super) fn effective_priority(&self) -> u64 {
        u64::from(self.priority.value()).saturating_add(self.waiting_turns)
    }
}
