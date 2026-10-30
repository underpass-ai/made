use super::{ProtocolWorkerPriority, ProtocolWorkerScheduleRequest, ProtocolWorkerWeight};

#[derive(Debug)]
pub(super) struct ProtocolWorkerRootQueue {
    pub(super) weight: ProtocolWorkerWeight,
    pub(super) priority: ProtocolWorkerPriority,
    pub(super) deficit: u64,
    pub(super) waiting_turns: u64,
    pub(super) pending: Vec<(u64, ProtocolWorkerScheduleRequest)>,
}

impl ProtocolWorkerRootQueue {
    pub(super) const fn new(
        weight: ProtocolWorkerWeight,
        priority: ProtocolWorkerPriority,
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
