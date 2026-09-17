//! Writing the rows a pre-stream store held.
//!
//! No production code writes `ceremony_instances` or `audit_journal`
//! any more — that is the point of A7 — so the tests that prove those
//! rows are still read, counted and importable have to put them there
//! the way the old engine did: straight into the two tables, one
//! snapshot and one chained record per session.

use made_core::entities::ceremony_events::CeremonyCompleted;
use made_core::entities::{
    AuditFact, AuditRecord, CeremonyDefinition, CeremonyEvent, CeremonyInstance,
};
use made_core::value_objects::{
    AuditActor, AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyRevision,
    CeremonyState, CeremonyTransition, CeremonyVersion, EventId, StateId, TransitionTrigger,
};
use time::OffsetDateTime;

use crate::engine::{Key, Table};
use crate::sqlite::keys::scoped;
use crate::sqlite::StoredCeremony;

use super::{encode, SqliteCeremonyStore};

pub(super) fn definition() -> CeremonyDefinition {
    CeremonyDefinition::new(
        CeremonyName::new("legacy_ceremony").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(StateId::new("OPEN").unwrap()),
            CeremonyState::terminal(StateId::new("DONE").unwrap()),
        ],
        vec![CeremonyTransition::new(
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
    store: &SqliteCeremonyStore,
    id: &CeremonyId,
) -> CeremonyInstance {
    let definition = definition();
    let instance = CeremonyInstance::start(
        id.clone(),
        &definition,
        CeremonyContext::empty(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .expect("required ceremony inputs");
    let record = AuditRecord::first(AuditFact {
        event_id: EventId::new(format!("{}:legacy", id.as_str())).unwrap(),
        event: CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
            final_state: StateId::new("DONE").unwrap(),
            completed_at: OffsetDateTime::UNIX_EPOCH,
        }),
        ceremony_id: id.clone(),
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
        Table::Ceremonies,
        Key::Str(id.as_str()),
        &encode(
            &StoredCeremony {
                revision: CeremonyRevision::INITIAL,
                instance: instance.clone(),
            },
            "encode ceremony",
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
