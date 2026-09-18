use made_core::entities::CeremonyEventReader;
use made_core::value_objects::{AuditEventType, EventSchemaVersion};
use serde_json::{json, Value};

fn transition() -> Value {
    let mut raw: Value = serde_json::from_str(include_str!(
        "fixtures/ceremony_events/v1/transition_applied.json"
    ))
    .unwrap();
    raw["transition"]["state_iteration"] = json!(2);
    raw["transition"]["state_visit"] = json!(3);
    raw["destination"] = json!({"state_visit": 4, "step_ids": ["a", "b"]});
    raw
}

#[test]
fn destination_reset_is_sealed_as_a_new_schema_and_rejects_contradictions() {
    let valid = transition();
    let event = CeremonyEventReader::read(
        AuditEventType::TransitionApplied,
        EventSchemaVersion::V3,
        valid.clone(),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(event).unwrap(), valid);
    for version in [EventSchemaVersion::V1, EventSchemaVersion::V2] {
        assert!(CeremonyEventReader::read(
            AuditEventType::TransitionApplied,
            version,
            valid.clone()
        )
        .is_err());
    }
    for mutation in [
        "missing_source",
        "missing_destination",
        "wrong_next",
        "duplicate_steps",
        "zero_visit",
        "missing_iteration",
    ] {
        let mut raw = valid.clone();
        match mutation {
            "missing_source" => {
                raw["transition"]
                    .as_object_mut()
                    .unwrap()
                    .remove("state_visit");
            }
            "missing_destination" => {
                raw.as_object_mut().unwrap().remove("destination");
            }
            "wrong_next" => raw["destination"]["state_visit"] = json!(3),
            "duplicate_steps" => raw["destination"]["step_ids"] = json!(["a", "a"]),
            "zero_visit" => raw["destination"]["state_visit"] = json!(0),
            "missing_iteration" => {
                raw["transition"]
                    .as_object_mut()
                    .unwrap()
                    .remove("state_iteration");
            }
            _ => unreachable!(),
        }
        assert!(
            CeremonyEventReader::read(
                AuditEventType::TransitionApplied,
                EventSchemaVersion::V3,
                raw
            )
            .is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn step_and_context_events_version_explicit_visits_without_reinterpreting_old_shapes() {
    for (kind, fixture, new_version) in [
        (
            AuditEventType::StepStarted,
            include_str!("fixtures/ceremony_events/v1/step_started.json"),
            EventSchemaVersion::V4,
        ),
        (
            AuditEventType::StepCompleted,
            include_str!("fixtures/ceremony_events/v1/step_completed.json"),
            EventSchemaVersion::V3,
        ),
        (
            AuditEventType::StepFailed,
            include_str!("fixtures/ceremony_events/v1/step_failed.json"),
            EventSchemaVersion::V3,
        ),
        (
            AuditEventType::ContextWritten,
            include_str!("fixtures/ceremony_events/v1/context_written.json"),
            EventSchemaVersion::V2,
        ),
    ] {
        let mut raw: Value = serde_json::from_str(fixture).unwrap();
        let legacy = CeremonyEventReader::read(kind, EventSchemaVersion::V1, raw.clone()).unwrap();
        assert_eq!(serde_json::to_value(legacy).unwrap(), raw);
        raw["state_iteration"] = json!(1);
        raw["state_visit"] = json!(2);
        let event = CeremonyEventReader::read(kind, new_version, raw.clone()).unwrap();
        assert_eq!(serde_json::to_value(event).unwrap(), raw);
        assert!(CeremonyEventReader::read(kind, EventSchemaVersion::V1, raw.clone()).is_err());
        raw.as_object_mut().unwrap().remove("state_iteration");
        assert!(CeremonyEventReader::read(kind, new_version, raw).is_err());
    }
}

#[test]
fn legacy_and_current_state_repeat_events_keep_their_distinct_shapes() {
    let mut raw = json!({"type": "state_iteration_started", "state_id": "A", "state_iteration": 2, "step_ids": ["a"], "started_at": "2026-09-18T09:00:00Z"});
    let legacy = CeremonyEventReader::read(
        AuditEventType::StateIterationStarted,
        EventSchemaVersion::V1,
        raw.clone(),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(legacy).unwrap(), raw);
    raw["state_visit"] = json!(3);
    let event = CeremonyEventReader::read(
        AuditEventType::StateIterationStarted,
        EventSchemaVersion::V2,
        raw.clone(),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(event).unwrap(), raw);
}
