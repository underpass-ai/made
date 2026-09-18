use serde::{Deserialize, Serialize};

use crate::entities::{Council, Deliberation, Statistics};
use crate::events::{
    DeliberationCompletedEvent, PhaseChangedEvent, TaskCompletedEvent, TaskDispatchedEvent,
    TaskFailedEvent,
};
use crate::ports::AgentDescriptor;
use crate::value_objects::{AgentId, EventId, OutputContract, OutputContractId, Specialty};

/// Council facts have their own durable order, separate from ceremony events.
/// A saved deliberation is explicitly a snapshot, never an invented phase history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "fact", rename_all = "snake_case")]
pub enum CouncilJournalEvent {
    CouncilRegistered(Council),
    CouncilReplaced(Council),
    CouncilDeleted(Specialty),
    AgentRegistered(AgentDescriptor),
    AgentUnregistered(AgentId),
    ContractRegistered(OutputContract),
    ContractDeleted(OutputContractId),
    DeliberationSnapshotSaved(Deliberation),
    StatisticsRecorded(Statistics),
    TaskDispatched(TaskDispatchedEvent),
    TaskCompleted(TaskCompletedEvent),
    TaskFailed(TaskFailedEvent),
    DeliberationCompleted(DeliberationCompletedEvent),
    PhaseChanged(PhaseChangedEvent),
}

impl CouncilJournalEvent {
    /// The caller's stable publication identity, if this is a messaging fact.
    #[must_use]
    pub fn publication_id(&self) -> Option<&EventId> {
        match self {
            Self::TaskDispatched(event) => Some(event.envelope().event_id()),
            Self::TaskCompleted(event) => Some(event.envelope().event_id()),
            Self::TaskFailed(event) => Some(event.envelope().event_id()),
            Self::DeliberationCompleted(event) => Some(event.envelope().event_id()),
            Self::PhaseChanged(event) => Some(event.envelope().event_id()),
            _ => None,
        }
    }
}
