//! Stored shapes pinned as fixtures.
//!
//! Two claims that must fall if broken: a record sealed under schema
//! version 1 — by the code that shipped before events travelled inside
//! records — still reads and verifies; and every event payload at
//! schema version 1 reads through the reader and re-serializes to the
//! bytes it was pinned with, so a payload cannot change shape without
//! bumping its version.

use made_core::entities::ceremony_events::InstanceImported;
use made_core::entities::{
    AuditChain, AuditFact, AuditRecord, CeremonyEvent, CeremonyEventReader, CeremonyInstance,
};
use made_core::value_objects::{
    AuditActor, AuditEventType, AuditRecordHash, CeremonyRevision, EventId, EventSchemaVersion,
    StateIteration,
};
use time::macros::datetime;

/// Two records sealed by the schema-version-1 code, captured verbatim.
const AUDIT_RECORDS_V1: &str = include_str!("fixtures/audit_record_v1.json");
const PRE_P2_EVENT_CHAIN: &str =
    include_str!("fixtures/audit_record_v2_event_schema_v1_chain.json");
const PRE_P5_IN_PROGRESS_SNAPSHOT: &str =
    include_str!("fixtures/legacy_in_progress_instance_pre_p5.json");

const EVERY_EVENT_TYPE: [AuditEventType; 24] = [
    AuditEventType::CeremonyDefinitionValidated,
    AuditEventType::CeremonyDefinitionPublished,
    AuditEventType::CeremonyInstanceStarted,
    AuditEventType::StepStarted,
    AuditEventType::StepCompleted,
    AuditEventType::StepFailed,
    AuditEventType::ContextWritten,
    AuditEventType::StateIterationStarted,
    AuditEventType::TransitionApplied,
    AuditEventType::InterventionRequested,
    AuditEventType::InterventionResponded,
    AuditEventType::InterventionClosed,
    AuditEventType::EvidenceCollected,
    AuditEventType::ParticipantsBound,
    AuditEventType::ReasonAsserted,
    AuditEventType::HumanApprovalRecorded,
    AuditEventType::HumanDeferralRecorded,
    AuditEventType::CeremonyCompleted,
    AuditEventType::CeremonyFailed,
    AuditEventType::InstanceImported,
    AuditEventType::MemoryRecalled,
    AuditEventType::ChildSpawnPlanned,
    AuditEventType::ChildSpawnPlanAdopted,
    AuditEventType::ChildCompletionAccepted,
];

/// The pinned version-1 payload of each event type a stream can hold.
///
/// Exhaustive on purpose: a new catalogue entry does not compile until
/// it either gets a golden or is named here as having no payload.
fn golden(event_type: AuditEventType) -> Option<&'static str> {
    match event_type {
        AuditEventType::CeremonyDefinitionValidated
        | AuditEventType::CeremonyDefinitionPublished
        | AuditEventType::CeremonyFailed
        | AuditEventType::StateIterationStarted
        | AuditEventType::CeremonyPaused
        | AuditEventType::CeremonyResumed
        | AuditEventType::CeremonyCancelled
        | AuditEventType::CeremonyDeadlineExceeded
        | AuditEventType::StateDeadlineExceeded
        | AuditEventType::StepDeadlineExceeded
        | AuditEventType::LateStepResultObserved => None,
        AuditEventType::ChildSpawnPlanned => Some(include_str!(
            "fixtures/ceremony_events/v1/child_spawn_planned.json"
        )),
        AuditEventType::ChildSpawnPlanAdopted => Some(include_str!(
            "fixtures/ceremony_events/v1/child_spawn_plan_adopted.json"
        )),
        AuditEventType::ChildCompletionAccepted => Some(include_str!(
            "fixtures/ceremony_events/v1/child_completion_accepted.json"
        )),
        AuditEventType::CeremonyInstanceStarted => Some(include_str!(
            "fixtures/ceremony_events/v1/ceremony_instance_started.json"
        )),
        AuditEventType::StepStarted => Some(include_str!(
            "fixtures/ceremony_events/v1/step_started.json"
        )),
        AuditEventType::StepCompleted => Some(include_str!(
            "fixtures/ceremony_events/v1/step_completed.json"
        )),
        AuditEventType::StepFailed => {
            Some(include_str!("fixtures/ceremony_events/v1/step_failed.json"))
        }
        AuditEventType::ContextWritten => Some(include_str!(
            "fixtures/ceremony_events/v1/context_written.json"
        )),
        AuditEventType::TransitionApplied => Some(include_str!(
            "fixtures/ceremony_events/v1/transition_applied.json"
        )),
        AuditEventType::InterventionRequested => Some(include_str!(
            "fixtures/ceremony_events/v1/intervention_requested.json"
        )),
        AuditEventType::InterventionResponded => Some(include_str!(
            "fixtures/ceremony_events/v1/intervention_responded.json"
        )),
        AuditEventType::InterventionClosed => Some(include_str!(
            "fixtures/ceremony_events/v1/intervention_closed.json"
        )),
        AuditEventType::EvidenceCollected => Some(include_str!(
            "fixtures/ceremony_events/v1/evidence_collected.json"
        )),
        AuditEventType::ParticipantsBound => Some(include_str!(
            "fixtures/ceremony_events/v1/participants_bound.json"
        )),
        AuditEventType::ReasonAsserted => Some(include_str!(
            "fixtures/ceremony_events/v1/reason_asserted.json"
        )),
        AuditEventType::HumanApprovalRecorded => Some(include_str!(
            "fixtures/ceremony_events/v1/human_approval_recorded.json"
        )),
        AuditEventType::HumanDeferralRecorded => Some(include_str!(
            "fixtures/ceremony_events/v1/human_deferral_recorded.json"
        )),
        AuditEventType::CeremonyCompleted => Some(include_str!(
            "fixtures/ceremony_events/v1/ceremony_completed.json"
        )),
        AuditEventType::InstanceImported => Some(include_str!(
            "fixtures/ceremony_events/v1/instance_imported.json"
        )),
        AuditEventType::MemoryRecalled => Some(include_str!(
            "fixtures/ceremony_events/v1/memory_recalled.json"
        )),
    }
}

#[test]
fn version_one_records_still_read_and_verify() {
    let records: Vec<AuditRecord> = serde_json::from_str(AUDIT_RECORDS_V1).unwrap();

    assert_eq!(records.len(), 2);
    for record in &records {
        assert_eq!(record.schema_version(), 1);
        assert!(record.event().is_none());
        assert!(record.event_schema_version().is_none());
        assert!(record.digest_is_intact().unwrap());
    }
    assert_eq!(
        records[0].event_type(),
        AuditEventType::CeremonyInstanceStarted
    );
    assert_eq!(records[1].event_type(), AuditEventType::StepCompleted);
    assert_eq!(
        records[1].correlation_id().map(EventId::as_str),
        Some("corr-1")
    );
    assert_eq!(
        records[1].trace_id(),
        Some("4bf92f3577b34da6a3ce929d0e0e4736")
    );
    assert!(AuditChain::verify(&records).is_intact());
}

#[test]
fn a_version_one_record_is_still_tamper_evident() {
    let mut json: serde_json::Value = serde_json::from_str(AUDIT_RECORDS_V1).unwrap();
    json[1]["actor"]["actor_id"] = "someone-else".into();
    let records: Vec<AuditRecord> = serde_json::from_value(json).unwrap();

    assert!(!AuditChain::verify(&records).is_intact());
}

#[test]
fn every_version_one_payload_reads_and_reserializes_unchanged() {
    let mut pinned = 0;
    for event_type in EVERY_EVENT_TYPE {
        let Some(text) = golden(event_type) else {
            continue;
        };
        let stored: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            stored["type"],
            event_type.as_str(),
            "{event_type:?}: the golden's tag is not the event type's wire name"
        );

        let event = CeremonyEventReader::read(event_type, EventSchemaVersion::V1, stored.clone())
            .unwrap_or_else(|error| panic!("{event_type:?}: {error}"));

        assert_eq!(event.event_type(), event_type);
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            stored,
            "{event_type:?}: re-serializes differently from its version-1 golden"
        );
        pinned += 1;
    }
    assert_eq!(pinned, 20);
}

#[test]
fn pre_p2_sealed_event_chain_keeps_hashes_and_absent_coordinates() {
    let records: Vec<AuditRecord> = serde_json::from_str(PRE_P2_EVENT_CHAIN).unwrap();

    assert_eq!(records.len(), 4);
    assert!(AuditChain::verify(&records).is_intact());
    for record in &records {
        assert_eq!(record.schema_version(), 2);
        assert_eq!(record.event_schema_version(), Some(EventSchemaVersion::V1));
        let event = serde_json::to_value(record.event().unwrap()).unwrap();
        assert!(event.get("state_iteration").is_none());
    }
}

#[test]
fn changed_payloads_require_coordinate_presence_to_match_their_version() {
    for event_type in [
        AuditEventType::StepStarted,
        AuditEventType::StepCompleted,
        AuditEventType::StepFailed,
        AuditEventType::TransitionApplied,
    ] {
        let old: serde_json::Value = serde_json::from_str(golden(event_type).unwrap()).unwrap();
        assert!(CeremonyEventReader::read(event_type, EventSchemaVersion::V1, old.clone()).is_ok());
        assert!(
            CeremonyEventReader::read(event_type, EventSchemaVersion::V2, old.clone()).is_err()
        );

        let mut current = old;
        if event_type == AuditEventType::TransitionApplied {
            current["transition"]["state_iteration"] = 1.into();
        } else {
            current["state_iteration"] = 1.into();
        }
        assert!(
            CeremonyEventReader::read(event_type, EventSchemaVersion::V1, current.clone()).is_err()
        );
        let event = CeremonyEventReader::read(event_type, EventSchemaVersion::V2, current)
            .unwrap_or_else(|error| panic!("{event_type:?}: {error}"));
        assert_eq!(event.schema_version(), EventSchemaVersion::V2);
    }
}

#[test]
fn a_legacy_imported_snapshot_defaults_every_state_coordinate_to_one() {
    let stored: serde_json::Value =
        serde_json::from_str(golden(AuditEventType::InstanceImported).unwrap()).unwrap();
    let event = CeremonyEventReader::read(
        AuditEventType::InstanceImported,
        EventSchemaVersion::V1,
        stored.clone(),
    )
    .unwrap();
    let CeremonyEvent::InstanceImported(imported) = event else {
        panic!("the pinned payload is an import");
    };

    assert_eq!(
        imported.snapshot.current_state_iteration(),
        StateIteration::FIRST
    );
    assert!(imported
        .snapshot
        .step_records()
        .values()
        .all(|record| record.state_iteration() == StateIteration::FIRST));
    assert_eq!(
        serde_json::to_value(CeremonyEvent::InstanceImported(imported)).unwrap(),
        stored,
        "reopening the legacy snapshot must not synthesize coordinates"
    );
}

#[test]
fn an_imported_pre_p5_in_progress_snapshot_keeps_its_payload_and_sealed_hash() {
    let literal: serde_json::Value = serde_json::from_str(PRE_P5_IN_PROGRESS_SNAPSHOT).unwrap();
    let snapshot: CeremonyInstance = serde_json::from_value(literal.clone()).unwrap();
    let ceremony_id = snapshot.id().clone();
    let definition_name = snapshot.definition_name().clone();
    let definition_version = snapshot.definition_version().clone();
    let event = CeremonyEvent::InstanceImported(InstanceImported {
        ceremony_id: ceremony_id.clone(),
        definition_name: definition_name.clone(),
        definition_version: definition_version.clone(),
        snapshot: Box::new(snapshot),
        legacy_journal_head_hash: Some(AuditRecordHash::from_bytes([9; 32])),
        legacy_revision: CeremonyRevision::INITIAL,
        imported_at: datetime!(2026-07-29 09:10:00 UTC),
    });
    let encoded_event = serde_json::to_value(&event).unwrap();
    assert_eq!(encoded_event["snapshot"], literal);
    assert!(encoded_event["snapshot"]["step_records"]["draft"]
        .get("claimed_role")
        .is_none());

    let record = AuditRecord::first(AuditFact {
        event_id: EventId::new("legacy-active:instance-imported:1").unwrap(),
        ceremony_id,
        definition_name,
        definition_version,
        occurred_at: datetime!(2026-07-29 09:10:00 UTC),
        actor: AuditActor::engine("made-migration").unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
        event,
    })
    .unwrap();
    assert_eq!(
        record.record_hash().to_string(),
        "281d21d6680d4a7443dc29136fd4025c6525236cc5304292aeb6070456b7e210"
    );
    assert!(record.digest_is_intact().unwrap());
    assert!(AuditChain::verify(std::slice::from_ref(&record)).is_intact());
    let round_trip: AuditRecord =
        serde_json::from_value(serde_json::to_value(&record).unwrap()).unwrap();
    assert_eq!(round_trip, record);
}
