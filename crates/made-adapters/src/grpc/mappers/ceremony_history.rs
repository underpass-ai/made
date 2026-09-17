//! What a ceremony left behind → proto: sealed records, the
//! transcript, the report, and the verdict on the chain that seals
//! them.
//!
//! A record goes out whole, field for field, with its payload as the
//! JSON the digest covers. That is what lets a client rebuild the
//! record and verify the chain on what it received; a rendering of a
//! record verifies nothing.

use made_app::usecases::{
    CeremonyEventPage, CeremonyJournalVerdict, CeremonyReport, PullCeremonyEventsOutput,
};
use made_core::entities::AuditRecord;
use made_core::error::DomainError;
use made_core::value_objects::{AuditSequence, CeremonyTranscript};
use made_proto::v1 as pb;
use time::format_description::well_known::Rfc3339;

use super::attributes::{attributes_to_struct, struct_from_json};

/// One page of a stream.
pub fn read_ceremony_events_response_from(
    page: &CeremonyEventPage,
) -> Result<pb::ReadCeremonyEventsResponse, DomainError> {
    Ok(pb::ReadCeremonyEventsResponse {
        records: page
            .records()
            .iter()
            .map(ceremony_event_record_from)
            .collect::<Result<Vec<_>, _>>()?,
        next_version: page.next_version().value(),
        head_version: page.head_version().value(),
    })
}

/// One page of a named global feed.
pub fn pull_ceremony_events_response_from(
    output: &PullCeremonyEventsOutput,
) -> Result<pb::PullCeremonyEventsResponse, DomainError> {
    Ok(pb::PullCeremonyEventsResponse {
        records: output
            .records()
            .iter()
            .map(|positioned| {
                Ok(pb::PositionedCeremonyEvent {
                    global_position: positioned.position.value(),
                    record: Some(ceremony_event_record_from(&positioned.record)?),
                })
            })
            .collect::<Result<Vec<_>, DomainError>>()?,
        acknowledged_through: output
            .acknowledged_through()
            .map(made_core::value_objects::GlobalPosition::value),
    })
}

/// One sealed record.
fn ceremony_event_record_from(
    record: &AuditRecord,
) -> Result<pb::CeremonyEventRecord, DomainError> {
    let event = match record.event() {
        Some(event) => {
            // The very bytes the digest was taken over. Serializing
            // fails only where the engine's own types cannot be
            // written, which is not the caller's doing.
            let json = serde_json::to_value(event).map_err(|_| DomainError::InvariantViolated {
                reason: "a sealed ceremony event cannot be rendered",
            })?;
            Some(
                struct_from_json(&json).ok_or(DomainError::InvariantViolated {
                    reason: "a sealed ceremony event is not an object",
                })?,
            )
        }
        None => None,
    };

    Ok(pb::CeremonyEventRecord {
        event_id: record.event_id().as_str().to_owned(),
        event_type: record.event_type().as_str().to_owned(),
        schema_version: record.schema_version(),
        ceremony_id: record.ceremony_id().as_str().to_owned(),
        definition_name: record.definition_name().as_str().to_owned(),
        definition_version: record.definition_version().as_str().to_owned(),
        sequence: record.sequence().value(),
        occurred_at: record.occurred_at().format(&Rfc3339).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "an audit record timestamp cannot be rendered canonically",
            }
        })?,
        actor: Some(pb::CeremonyEventActor {
            actor_id: record.actor().actor_id().to_owned(),
            kind: record.actor().kind().as_str().to_owned(),
            role_id: record
                .actor()
                .role_id()
                .map(|role| role.as_str().to_owned())
                .unwrap_or_default(),
        }),
        correlation_id: record
            .correlation_id()
            .map(|id| id.as_str().to_owned())
            .unwrap_or_default(),
        causation_id: record
            .causation_id()
            .map(|id| id.as_str().to_owned())
            .unwrap_or_default(),
        trace_id: record.trace_id().unwrap_or_default().to_owned(),
        event_schema_version: record
            .event_schema_version()
            .map_or(0, made_core::value_objects::EventSchemaVersion::get),
        event,
        previous_record_hash: record
            .previous_record_hash()
            .map(|hash| hash.as_bytes().to_vec())
            .unwrap_or_default(),
        record_hash: record.record_hash().as_bytes().to_vec(),
    })
}

/// The ordered contributions of one session.
#[must_use]
pub fn get_ceremony_transcript_response_from(
    transcript: &CeremonyTranscript,
) -> pb::GetCeremonyTranscriptResponse {
    pb::GetCeremonyTranscriptResponse {
        entries: transcript
            .contributions()
            .iter()
            .map(|contribution| pb::CeremonyTranscriptEntry {
                step_id: contribution.step_id().as_str().to_owned(),
                role_id: contribution.role_id().as_str().to_owned(),
                output: Some(attributes_to_struct(contribution.output().attributes())),
            })
            .collect(),
    }
}

/// The report and what it was projected from.
#[must_use]
pub fn generate_ceremony_report_response_from(
    report: &CeremonyReport,
) -> pb::GenerateCeremonyReportResponse {
    pb::GenerateCeremonyReportResponse {
        report_markdown: report.markdown().to_owned(),
        ceremony_ids: report
            .bindings()
            .iter()
            .map(|binding| binding.ceremony_id().as_str().to_owned())
            .collect(),
        ceremony_count: u32::try_from(report.ceremony_count()).unwrap_or(u32::MAX),
        completed_count: u32::try_from(report.completed_count()).unwrap_or(u32::MAX),
        incomplete_count: u32::try_from(report.incomplete_count()).unwrap_or(u32::MAX),
        definition_bindings: report
            .bindings()
            .iter()
            .map(|binding| pb::CeremonyReportBinding {
                ceremony_id: binding.ceremony_id().as_str().to_owned(),
                definition_name: binding.definition_name().as_str().to_owned(),
                definition_version: binding.definition_version().as_str().to_owned(),
                definition_digest: binding.definition_digest().to_hex(),
                bound_definition_digest: binding
                    .bound_definition_digest()
                    .map(|digest| digest.to_hex())
                    .unwrap_or_default(),
            })
            .collect(),
    }
}

/// The verdict on one journal's chain.
///
/// Absent is zero and empty here, as everywhere else in proto3: a
/// position counts from one, so zero cannot be a position, and an
/// intact journal has no reason to give.
pub fn verify_ceremony_journal_response_from(
    verdict: &CeremonyJournalVerdict,
) -> pb::VerifyCeremonyJournalResponse {
    pb::VerifyCeremonyJournalResponse {
        ceremony_id: verdict.ceremony_id().as_str().to_owned(),
        head_version: verdict.head_version().value(),
        record_count: u32::try_from(verdict.record_count()).unwrap_or(u32::MAX),
        intact: verdict.is_intact(),
        first_broken_sequence: verdict
            .first_broken_sequence()
            .map_or(0, AuditSequence::value),
        reason: verdict.reason().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use made_core::entities::ceremony_events::CeremonyInstanceStarted;
    use made_core::entities::{AuditFact, CeremonyEvent};
    use made_core::value_objects::{
        Attributes, AuditActor, AuditActorKind, AuditChainDefect, AuditChainVerdict,
        CeremonyContext, CeremonyId, CeremonyName, CeremonyStepContribution, CeremonyVersion,
        EventId, RoleId, StateId, StepId, StepOutput, StreamVersion,
    };
    use prost_types::value::Kind;
    use time::macros::datetime;
    use time::OffsetDateTime;

    use super::*;

    const AT: OffsetDateTime = datetime!(2026-09-16 09:00:00 UTC);

    fn record() -> AuditRecord {
        let ceremony_id = CeremonyId::new("session-1").unwrap();
        AuditRecord::first(AuditFact {
            event_id: EventId::new("e1").unwrap(),
            event: CeremonyEvent::CeremonyInstanceStarted(CeremonyInstanceStarted {
                ceremony_id: ceremony_id.clone(),
                definition_name: CeremonyName::new("planning_ceremony").unwrap(),
                definition_version: CeremonyVersion::v1(),
                initial_state: StateId::new("OPEN").unwrap(),
                step_ids: BTreeSet::from([StepId::new("draft").unwrap()]),
                context: CeremonyContext::empty(),
                bound_definition: None,
                created_at: AT,
            }),
            ceremony_id,
            definition_name: CeremonyName::new("planning_ceremony").unwrap(),
            definition_version: CeremonyVersion::v1(),
            occurred_at: AT,
            actor: AuditActor::new(
                "operator-1",
                AuditActorKind::Service,
                Some(RoleId::new("FACILITATOR").unwrap()),
            )
            .unwrap(),
            correlation_id: Some(EventId::new("e1").unwrap()),
            causation_id: None,
            trace: None,
        })
        .unwrap()
    }

    #[test]
    fn a_record_travels_whole_with_its_digest_and_its_payload() {
        let record = record();
        let wire = ceremony_event_record_from(&record).unwrap();

        assert_eq!(wire.event_id, "e1");
        assert_eq!(wire.event_type, "ceremony_instance_started");
        assert_eq!(wire.schema_version, 2);
        assert_eq!(wire.sequence, 1);
        assert_eq!(wire.occurred_at, "2026-09-16T09:00:00Z");
        assert_eq!(wire.event_schema_version, 1);
        let actor = wire.actor.as_ref().unwrap();
        assert_eq!(actor.actor_id, "operator-1");
        assert_eq!(actor.kind, "service");
        assert_eq!(actor.role_id, "FACILITATOR");
        assert_eq!(wire.correlation_id, "e1");
        // An absent optional is an empty field on the wire; proto3 has
        // no other way to say it for a string.
        assert!(wire.causation_id.is_empty());
        assert!(wire.trace_id.is_empty());
        assert!(wire.previous_record_hash.is_empty());
        assert_eq!(wire.record_hash, record.record_hash().as_bytes().to_vec());
        assert_eq!(wire.record_hash.len(), 32);

        // The payload is the sealed JSON, tag included: what the
        // digest covers, not a summary of it.
        let event = wire.event.as_ref().unwrap();
        assert_eq!(
            event
                .fields
                .get("type")
                .and_then(|value| value.kind.clone()),
            Some(Kind::StringValue("ceremony_instance_started".to_owned()))
        );
        assert_eq!(
            event
                .fields
                .get("initial_state")
                .and_then(|value| value.kind.clone()),
            Some(Kind::StringValue("OPEN".to_owned()))
        );
    }

    #[test]
    fn a_transcript_entry_carries_its_seat_and_its_output() {
        let transcript = CeremonyTranscript::new(vec![CeremonyStepContribution::new(
            StepId::new("draft").unwrap(),
            RoleId::new("WRITER").unwrap(),
            StepOutput::new(
                Attributes::new(BTreeMap::from([(
                    "note".to_owned(),
                    serde_json::json!("done"),
                )]))
                .unwrap(),
            ),
        )]);

        let wire = get_ceremony_transcript_response_from(&transcript);

        assert_eq!(wire.entries.len(), 1);
        assert_eq!(wire.entries[0].step_id, "draft");
        assert_eq!(wire.entries[0].role_id, "WRITER");
        assert_eq!(
            wire.entries[0]
                .output
                .as_ref()
                .unwrap()
                .fields
                .get("note")
                .and_then(|value| value.kind.clone()),
            Some(Kind::StringValue("done".to_owned()))
        );
    }

    #[test]
    fn an_intact_journal_reports_no_position_and_no_reason() {
        let verdict = CeremonyJournalVerdict::new(
            CeremonyId::new("session-1").unwrap(),
            StreamVersion::new(4),
            4,
            AuditChainVerdict::Intact,
        );

        let wire = verify_ceremony_journal_response_from(&verdict);

        assert_eq!(wire.ceremony_id, "session-1");
        assert_eq!(wire.head_version, 4);
        assert_eq!(wire.record_count, 4);
        assert!(wire.intact);
        assert_eq!(wire.first_broken_sequence, 0);
        assert!(wire.reason.is_empty());
    }

    #[test]
    fn a_broken_journal_carries_the_first_position_and_why() {
        let verdict = CeremonyJournalVerdict::new(
            CeremonyId::new("session-1").unwrap(),
            StreamVersion::new(4),
            3,
            AuditChainVerdict::Broken(AuditChainDefect::DigestAltered {
                at: AuditSequence::new(2).unwrap(),
            }),
        );

        let wire = verify_ceremony_journal_response_from(&verdict);

        assert!(!wire.intact);
        assert_eq!(wire.first_broken_sequence, 2);
        assert!(wire.reason.contains("digest"), "{}", wire.reason);
        assert_eq!(wire.record_count, 3);
    }
}
