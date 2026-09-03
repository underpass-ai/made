use time::OffsetDateTime;

use crate::value_objects::{
    AuditActor, AuditEventType, CeremonyId, CeremonyName, CeremonyVersion, EventId, TraceContext,
};

/// Everything an audit record states before the journal assigns its position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFact {
    pub event_id: EventId,
    pub event_type: AuditEventType,
    pub ceremony_id: CeremonyId,
    pub definition_name: CeremonyName,
    pub definition_version: CeremonyVersion,
    pub occurred_at: OffsetDateTime,
    pub actor: AuditActor,
    pub correlation_id: Option<EventId>,
    pub causation_id: Option<EventId>,
    pub trace: Option<TraceContext>,
}
