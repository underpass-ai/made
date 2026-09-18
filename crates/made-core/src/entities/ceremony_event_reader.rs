//! [`CeremonyEventReader`] — where a stored payload becomes a typed
//! event, and where old shapes will be brought forward.

use crate::error::DomainError;
use crate::value_objects::{AuditEventType, EventSchemaVersion};

use super::CeremonyEvent;

/// Reads a sealed payload back as the event its record names.
///
/// The one seam through which raw event JSON enters the domain. A
/// payload is read under the schema version its record was sealed
/// with; when a payload shape changes, its new version deserializes
/// directly and the old one gets an upcaster here, so records written
/// under earlier shapes keep reading. State-repeat coordinates are the
/// first versioned evolution: affected payloads use version 2 when the
/// coordinate is explicitly present, while their version-1 shape remains
/// readable without it.
#[derive(Debug)]
pub struct CeremonyEventReader;

impl CeremonyEventReader {
    /// Read `raw` as the event `event_type` at payload version `version`.
    ///
    /// Refuses a version no reader exists for, a payload that does not
    /// deserialize as an event, and a payload whose tag names a
    /// different type than the record does — each naming the type and
    /// version so an operator can tell which record and which reader
    /// disagree.
    pub fn read(
        event_type: AuditEventType,
        version: EventSchemaVersion,
        raw: serde_json::Value,
    ) -> Result<CeremonyEvent, DomainError> {
        let supported = match version {
            EventSchemaVersion::V1 => true,
            EventSchemaVersion::V2 => matches!(
                event_type,
                AuditEventType::CeremonyInstanceStarted
                    | AuditEventType::StepStarted
                    | AuditEventType::StepCompleted
                    | AuditEventType::StepFailed
                    | AuditEventType::TransitionApplied
                    | AuditEventType::ContextWritten
                    | AuditEventType::StateIterationStarted
            ),
            EventSchemaVersion::V3 => matches!(
                event_type,
                AuditEventType::StepStarted
                    | AuditEventType::StepCompleted
                    | AuditEventType::StepFailed
                    | AuditEventType::TransitionApplied
            ),
            EventSchemaVersion::V4 => matches!(
                event_type,
                AuditEventType::StepStarted | AuditEventType::StepFailed
            ),
            EventSchemaVersion::V5 => event_type == AuditEventType::StepStarted,
            _ => false,
        };
        if !supported {
            return Err(unreadable(
                event_type,
                version,
                "no reader exists for this schema version",
            ));
        }
        let event = serde_json::from_value::<CeremonyEvent>(raw).map_err(|_| {
            unreadable(
                event_type,
                version,
                "the payload does not deserialize as a ceremony event",
            )
        })?;
        if event.event_type() != event_type {
            return Err(unreadable(
                event_type,
                version,
                "the payload's tag names a different event type",
            ));
        }
        validate_seals(&event, event_type, version)?;
        let coordinates_valid = match &event {
            CeremonyEvent::StepStarted(e) => e.state_visit.is_none() || e.state_iteration.is_some(),
            CeremonyEvent::StepCompleted(e) => {
                e.state_visit.is_none() || e.state_iteration.is_some()
            }
            CeremonyEvent::StepFailed(e) => e.state_visit.is_none() || e.state_iteration.is_some(),
            CeremonyEvent::TransitionApplied(e) => match &e.destination {
                Some(destination) => {
                    e.transition.has_explicit_state_visit()
                        && e.transition.has_explicit_state_iteration()
                        && e.transition.state_visit().next().ok() == Some(destination.state_visit)
                        && destination
                            .step_ids
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            == destination.step_ids.len()
                }
                None => !e.transition.has_explicit_state_visit(),
            },
            _ => true,
        };
        if !coordinates_valid {
            return Err(unreadable(
                event_type,
                version,
                "inconsistent state visit coordinates or destination reset",
            ));
        }
        if event.schema_version() != version {
            return Err(unreadable(
                event_type,
                version,
                "the payload shape does not match its schema version",
            ));
        }
        Ok(event)
    }
}

fn unreadable(
    event_type: AuditEventType,
    version: EventSchemaVersion,
    reason: &'static str,
) -> DomainError {
    DomainError::UnreadableCeremonyEvent {
        event_type: event_type.as_str(),
        version: version.get(),
        reason,
    }
}

fn validate_seals(
    event: &CeremonyEvent,
    event_type: AuditEventType,
    version: EventSchemaVersion,
) -> Result<(), DomainError> {
    if let CeremonyEvent::StepCompleted(completed) = event {
        if completed.result.failure_kind().is_some() {
            return Err(unreadable(
                event_type,
                version,
                "step completion cannot carry a failure kind",
            ));
        }
    }
    if let CeremonyEvent::StepFailed(failed) = event {
        if failed.result.failure_kind().is_some()
            && failed.result.status() != crate::value_objects::StepStatus::Failed
        {
            return Err(unreadable(
                event_type,
                version,
                "only failed results carry a failure kind",
            ));
        }
    }
    if let CeremonyEvent::StepStarted(started) = event {
        if started.role_from.is_some() && started.sealed_role.is_some() {
            return Err(unreadable(
                event_type,
                version,
                "a step start cannot carry both dynamic and static role seals",
            ));
        }
        if started
            .sealed_role
            .as_ref()
            .is_some_and(|sealed| sealed != &started.started_by)
        {
            return Err(unreadable(
                event_type,
                version,
                "the sealed static role differs from started_by",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::ceremony_events::CeremonyCompleted;
    use crate::value_objects::StateId;
    use serde_json::json;
    use time::macros::datetime;

    fn completed_json() -> serde_json::Value {
        json!({
            "type": "ceremony_completed",
            "final_state": "DONE",
            "completed_at": "2026-07-29T09:00:00Z",
        })
    }

    fn step_started_json() -> serde_json::Value {
        json!({
            "type": "step_started",
            "step_id": "draft",
            "iteration": 1,
            "attempt": 1,
            "lease": {
                "owner_id": "host-1",
                "idempotency_key": "ceremony-1:draft:1",
                "acquired_at": "2026-07-29T09:00:00Z",
                "expires_at": "2026-07-29T09:01:00Z"
            },
            "started_by": "writer",
            "started_at": "2026-07-29T09:00:00Z"
        })
    }

    #[test]
    fn version_one_reads_directly() {
        let event = CeremonyEventReader::read(
            AuditEventType::CeremonyCompleted,
            EventSchemaVersion::V1,
            completed_json(),
        )
        .unwrap();

        assert_eq!(
            event,
            CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
                final_state: StateId::new("DONE").unwrap(),
                completed_at: datetime!(2026-07-29 09:00:00 UTC),
            })
        );
    }

    #[test]
    fn completed_event_rejects_injected_failure_classification_at_every_readable_version() {
        let legacy: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/ceremony_events/v1/step_completed.json"
        ))
        .unwrap();
        for version in [
            EventSchemaVersion::V1,
            EventSchemaVersion::V2,
            EventSchemaVersion::V3,
        ] {
            let mut raw = legacy.clone();
            if version != EventSchemaVersion::V1 {
                raw["state_iteration"] = json!(1);
            }
            if version == EventSchemaVersion::V3 {
                raw["state_visit"] = json!(1);
            }
            CeremonyEventReader::read(AuditEventType::StepCompleted, version, raw.clone())
                .expect("unclassified completion remains readable");
            raw["result"]["failure_kind"] = json!("no_valid_proposal");
            assert!(matches!(
                CeremonyEventReader::read(AuditEventType::StepCompleted, version, raw),
                Err(DomainError::UnreadableCeremonyEvent {
                    reason: "step completion cannot carry a failure kind",
                    ..
                })
            ));
        }
    }

    #[test]
    fn an_unknown_schema_version_is_refused_by_name() {
        let error = CeremonyEventReader::read(
            AuditEventType::CeremonyCompleted,
            EventSchemaVersion::new(2).unwrap(),
            completed_json(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DomainError::UnreadableCeremonyEvent {
                event_type: "ceremony_completed",
                version: 2,
                ..
            }
        ));
        assert!(error.to_string().contains("ceremony_completed"));
        assert!(error.to_string().contains("version 2"));
    }

    #[test]
    fn a_tag_that_disagrees_with_the_record_is_refused() {
        let error = CeremonyEventReader::read(
            AuditEventType::StepCompleted,
            EventSchemaVersion::V1,
            completed_json(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DomainError::UnreadableCeremonyEvent {
                event_type: "step_completed",
                version: 1,
                ..
            }
        ));
    }

    #[test]
    fn a_payload_that_is_not_an_event_is_refused() {
        let error = CeremonyEventReader::read(
            AuditEventType::CeremonyCompleted,
            EventSchemaVersion::V1,
            json!({ "type": "ceremony_completed" }),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DomainError::UnreadableCeremonyEvent {
                event_type: "ceremony_completed",
                ..
            }
        ));
    }

    #[test]
    fn version_one_refuses_a_version_two_coordinate() {
        let mut raw = step_started_json();
        raw["state_iteration"] = json!(1);

        let error =
            CeremonyEventReader::read(AuditEventType::StepStarted, EventSchemaVersion::V1, raw)
                .unwrap_err();

        assert!(matches!(
            error,
            DomainError::UnreadableCeremonyEvent {
                event_type: "step_started",
                version: 1,
                ..
            }
        ));
    }

    #[test]
    fn version_two_requires_an_explicit_coordinate() {
        let error = CeremonyEventReader::read(
            AuditEventType::StepStarted,
            EventSchemaVersion::V2,
            step_started_json(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DomainError::UnreadableCeremonyEvent {
                event_type: "step_started",
                version: 2,
                ..
            }
        ));
    }

    #[test]
    fn version_three_refuses_contradictory_static_role_seals() {
        let mut raw = step_started_json();
        raw["state_iteration"] = json!(1);
        raw["sealed_role"] = json!("reviewer");

        assert!(CeremonyEventReader::read(
            AuditEventType::StepStarted,
            EventSchemaVersion::V3,
            raw,
        )
        .is_err());
    }

    #[test]
    fn version_three_refuses_two_role_seal_markers() {
        let mut raw = step_started_json();
        raw["state_iteration"] = json!(1);
        raw["role_from"] = json!("next_role");
        raw["sealed_role"] = json!("writer");

        assert!(CeremonyEventReader::read(
            AuditEventType::StepStarted,
            EventSchemaVersion::V3,
            raw,
        )
        .is_err());
    }
}
