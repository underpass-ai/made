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
        let event = match version {
            EventSchemaVersion::V1 => {
                serde_json::from_value::<CeremonyEvent>(raw).map_err(|_| {
                    unreadable(
                        event_type,
                        version,
                        "the payload does not deserialize as a ceremony event",
                    )
                })?
            }
            EventSchemaVersion::V2
                if matches!(
                    event_type,
                    AuditEventType::StepStarted
                        | AuditEventType::StepCompleted
                        | AuditEventType::StepFailed
                        | AuditEventType::TransitionApplied
                ) =>
            {
                serde_json::from_value::<CeremonyEvent>(raw).map_err(|_| {
                    unreadable(
                        event_type,
                        version,
                        "the payload does not deserialize as a ceremony event",
                    )
                })?
            }
            other => {
                return Err(unreadable(
                    event_type,
                    other,
                    "no reader exists for this schema version",
                ))
            }
        };
        if event.event_type() != event_type {
            return Err(unreadable(
                event_type,
                version,
                "the payload's tag names a different event type",
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
}
