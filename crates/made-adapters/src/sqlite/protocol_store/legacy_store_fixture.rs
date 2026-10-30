//! Writing the rows a pre-stream store held.
//!
//! No production code writes `protocol_instances` or `audit_journal`
//! any more — that is the point of A7 — so the tests that prove those
//! rows are still read, counted and importable have to put them there
//! the way the old engine did: straight into the two tables, one
//! snapshot and one chained record per session.

use made_core::entities::protocol_events::ProtocolCompleted;
use made_core::entities::{
    AuditFact, AuditRecord, ProtocolDefinition, ProtocolEvent, ProtocolInstance,
};
use made_core::value_objects::{
    AuditActor, AuditActorKind, EventId, ProtocolContext, ProtocolId, ProtocolName,
    ProtocolRevision, ProtocolState, ProtocolTransition, ProtocolVersion, StateId,
    TransitionTrigger,
};
use time::OffsetDateTime;

use crate::engine::{Key, Table};
use crate::sqlite::keys::scoped;
use crate::sqlite::StoredProtocol;

use super::{encode, SqliteProtocolStore};

pub(super) fn definition() -> ProtocolDefinition {
    ProtocolDefinition::new(
        ProtocolName::new("legacy_protocol").unwrap(),
        ProtocolVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            ProtocolState::initial(StateId::new("OPEN").unwrap()),
            ProtocolState::terminal(StateId::new("DONE").unwrap()),
        ],
        vec![ProtocolTransition::new(
            StateId::new("OPEN").unwrap(),
            StateId::new("DONE").unwrap(),
            TransitionTrigger::new("finish").unwrap(),
            Vec::new(),
        )
        .unwrap()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap()
}

/// A session and one journal record, in the two tables v0.3.x wrote
/// them to and with no stream beside them.
pub(super) fn write_legacy_instance(
    store: &SqliteProtocolStore,
    id: &ProtocolId,
) -> ProtocolInstance {
    let definition = definition();
    let instance = ProtocolInstance::start(
        id.clone(),
        &definition,
        ProtocolContext::empty(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .expect("required protocol inputs");
    let record = AuditRecord::first(AuditFact {
        event_id: EventId::new(format!("{}:legacy", id.as_str())).unwrap(),
        event: ProtocolEvent::ProtocolCompleted(ProtocolCompleted {
            final_state: StateId::new("DONE").unwrap(),
            completed_at: OffsetDateTime::UNIX_EPOCH,
        }),
        protocol_id: id.clone(),
        definition_name: definition.name().clone(),
        definition_version: definition.version().clone(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("legacy", AuditActorKind::Engine, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    })
    .unwrap();

    let mut tx = store.engine.begin_write().unwrap();
    tx.insert(
        Table::Protocols,
        Key::Str(id.as_str()),
        &encode(
            &StoredProtocol {
                revision: ProtocolRevision::INITIAL,
                instance: instance.clone(),
            },
            "encode protocol",
        )
        .unwrap(),
    )
    .unwrap();
    tx.insert(
        Table::Journal,
        Key::Bytes(&scoped(id, record.sequence().value())),
        &encode(&record, "encode audit record").unwrap(),
    )
    .unwrap();
    tx.commit().unwrap();
    instance
}
