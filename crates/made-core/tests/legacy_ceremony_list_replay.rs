//! Tightening command lists must not invalidate an accepted version-1 journal.

use made_core::entities::{CeremonyEventReader, CeremonyInstance};
use made_core::value_objects::{AuditEventType, EventSchemaVersion};
use serde_json::{json, Value};

#[test]
fn legacy_deferrals_and_interventions_replay_without_truncation_or_deduplication() {
    let start: Value = serde_json::from_str(include_str!(
        "fixtures/ceremony_events/v1/ceremony_instance_started.json"
    ))
    .unwrap();
    let started = CeremonyEventReader::read(
        AuditEventType::CeremonyInstanceStarted,
        EventSchemaVersion::V1,
        start,
    )
    .unwrap();
    let mut deferral: Value = serde_json::from_str(include_str!(
        "fixtures/ceremony_events/v1/human_deferral_recorded.json"
    ))
    .unwrap();
    let conditions = vec!["same legacy condition"; 101];
    deferral["deferral"]["content"]["reconsider_when"] = json!(conditions);
    let deferred = CeremonyEventReader::read(
        AuditEventType::HumanDeferralRecorded,
        EventSchemaVersion::V1,
        deferral.clone(),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&deferred).unwrap(), deferral);

    let mut intervention: Value = serde_json::from_str(include_str!(
        "fixtures/ceremony_events/v1/intervention_requested.json"
    ))
    .unwrap();
    let roles = (0..101).map(|i| format!("ROLE-{i:03}")).collect::<Vec<_>>();
    intervention["intervention"]["target"]["role_ids"] = json!(roles);
    let requested = CeremonyEventReader::read(
        AuditEventType::InterventionRequested,
        EventSchemaVersion::V1,
        intervention.clone(),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&requested).unwrap(), intervention);

    let instance = CeremonyInstance::rehydrate([&started, &deferred, &requested]).unwrap();
    assert_eq!(
        instance.guard_deferrals()[0].content().reconsider_when(),
        conditions
    );
    assert_eq!(
        instance.interventions()[0]
            .target()
            .role_ids()
            .unwrap()
            .len(),
        101
    );
}
