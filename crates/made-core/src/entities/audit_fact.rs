use time::OffsetDateTime;

use crate::entities::ProtocolEvent;
use crate::value_objects::{
    AuditActor, AuthorizationEvidence, EventId, ProtocolId, ProtocolName, ProtocolVersion,
    TraceContext,
};

use super::AuthorizedAuditFact;

/// Everything an audit record states before the journal assigns its position.
///
/// The event is the fact itself, payload included. The type a record
/// names is derived from it (`fact.event.event_type()`) rather than
/// declared beside it, so a fact cannot claim one thing and carry
/// another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFact {
    pub event_id: EventId,
    pub event: ProtocolEvent,
    pub protocol_id: ProtocolId,
    pub definition_name: ProtocolName,
    pub definition_version: ProtocolVersion,
    pub occurred_at: OffsetDateTime,
    pub actor: AuditActor,
    pub correlation_id: Option<EventId>,
    pub causation_id: Option<EventId>,
    pub trace: Option<TraceContext>,
}

impl AuditFact {
    /// Bind a successful admission decision to this fact.
    ///
    /// The wrapper makes authorization-bearing records explicit without adding
    /// an optional field to every historical and unauthorised fact builder.
    #[must_use]
    pub fn authorized(self, evidence: AuthorizationEvidence) -> AuthorizedAuditFact {
        AuthorizedAuditFact::new(self, evidence)
    }
}
