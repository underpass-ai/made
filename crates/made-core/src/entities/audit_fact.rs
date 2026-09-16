use time::OffsetDateTime;

use crate::entities::CeremonyEvent;
use crate::value_objects::{
    AuditActor, CeremonyId, CeremonyName, CeremonyVersion, EventId, TraceContext,
};

/// Everything an audit record states before the journal assigns its position.
///
/// The event is the fact itself, payload included. The type a record
/// names is derived from it (`fact.event.event_type()`) rather than
/// declared beside it, so a fact cannot claim one thing and carry
/// another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFact {
    pub event_id: EventId,
    pub event: CeremonyEvent,
    pub ceremony_id: CeremonyId,
    pub definition_name: CeremonyName,
    pub definition_version: CeremonyVersion,
    pub occurred_at: OffsetDateTime,
    pub actor: AuditActor,
    pub correlation_id: Option<EventId>,
    pub causation_id: Option<EventId>,
    pub trace: Option<TraceContext>,
}
