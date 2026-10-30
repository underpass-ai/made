//! A session that was never started here still folds.
//!
//! Three claims that must fall if broken: an imported session is the
//! snapshot it carried, a stream may open with an import, and an
//! import anywhere else is refused rather than silently applied.

use made_core::entities::protocol_events::{InstanceImported, ProtocolCompleted};
use made_core::entities::{ProtocolEvent, ProtocolInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    AuditEventType, AuditRecordHash, EventSchemaVersion, ProtocolRevision, StateId,
};

use super::fixture::{at, definition, opened};

fn imported_event(snapshot: &ProtocolInstance) -> ProtocolEvent {
    ProtocolEvent::InstanceImported(InstanceImported {
        protocol_id: snapshot.id().clone(),
        definition_name: snapshot.definition_name().clone(),
        definition_version: snapshot.definition_version().clone(),
        snapshot: Box::new(snapshot.clone()),
        legacy_journal_head_hash: Some(AuditRecordHash::from_bytes([3_u8; 32])),
        legacy_revision: ProtocolRevision::INITIAL,
        imported_at: at(10),
    })
}

fn completed() -> ProtocolEvent {
    ProtocolEvent::ProtocolCompleted(ProtocolCompleted {
        final_state: StateId::new("CLOSED").unwrap(),
        completed_at: at(20),
    })
}

#[test]
fn the_import_names_itself_in_the_catalogue_at_version_one() {
    let event = imported_event(&opened(&definition()));

    assert_eq!(event.event_type(), AuditEventType::InstanceImported);
    assert_eq!(event.event_type().as_str(), "instance_imported");
    assert_eq!(event.schema_version(), EventSchemaVersion::V1);
}

#[test]
fn a_stream_that_opens_with_an_import_folds_to_the_snapshot_it_carried() {
    let snapshot = opened(&definition());

    let folded = ProtocolInstance::rehydrate([&imported_event(&snapshot)]).unwrap();

    assert_eq!(folded, snapshot);
}

#[test]
fn what_follows_an_import_is_applied_to_the_imported_session() {
    let snapshot = opened(&definition());

    let folded = ProtocolInstance::rehydrate([&imported_event(&snapshot), &completed()]).unwrap();

    assert_eq!(folded.completed_at(), Some(at(20)));
    assert_eq!(folded.id(), snapshot.id());
}

#[test]
fn an_import_after_the_first_position_is_refused() {
    let snapshot = opened(&definition());
    // `decide_start` returns the opening batch: the start, and the
    // recollection when the session was told something. This one
    // declares no scope, so the batch is the start alone.
    let opening = match ProtocolInstance::decide_start(
        snapshot.id().clone(),
        &definition(),
        snapshot.context().clone(),
        None,
        at(0),
    )
    .expect("required protocol inputs")
    .into_iter()
    .next()
    {
        Some(opening @ ProtocolEvent::ProtocolInstanceStarted(_)) => opening,
        other => panic!("starting decides its own opening, got {other:?}"),
    };

    let refused = ProtocolInstance::rehydrate([&opening, &imported_event(&snapshot)]);

    assert!(
        matches!(
            refused,
            Err(DomainError::InvariantViolated {
                reason: "a protocol stream carries an import only as its first event"
            })
        ),
        "an import in the middle of a stream must be refused, got {refused:?}"
    );
}

/// Applying it out of order does replace the session — which is why
/// `rehydrate` is where the position is enforced, and not `apply`.
#[test]
fn applying_an_import_replaces_whatever_the_session_was() {
    let mut instance = opened(&definition());
    instance.apply(&completed());
    assert!(instance.completed_at().is_some());

    let snapshot = opened(&definition());
    instance.apply(&imported_event(&snapshot));

    assert_eq!(instance, snapshot);
}
