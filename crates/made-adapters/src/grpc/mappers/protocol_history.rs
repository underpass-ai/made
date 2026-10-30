//! What a protocol left behind → proto: sealed records, the
//! transcript, the report, and the verdict on the chain that seals
//! them.
//!
//! A record goes out whole, field for field, with its payload as the
//! JSON the digest covers. That is what lets a client rebuild the
//! record and verify the chain on what it received; a rendering of a
//! record verifies nothing.

use made_app::usecases::{
    ProtocolEventPage, ProtocolJournalVerdict, ProtocolReport, PullProtocolEventsOutput,
};
use made_core::entities::AuditRecord;
use made_core::error::DomainError;
use made_core::value_objects::{AuditSequence, ProtocolTranscript};
use made_proto::v1 as pb;
use time::format_description::well_known::Rfc3339;

use super::attributes::{attributes_to_struct, struct_from_json};

/// One page of a stream.
pub fn read_protocol_events_response_from(
    page: &ProtocolEventPage,
) -> Result<pb::ReadProtocolEventsResponse, DomainError> {
    Ok(pb::ReadProtocolEventsResponse {
        records: page
            .records()
            .iter()
            .map(protocol_event_record_from)
            .collect::<Result<Vec<_>, _>>()?,
        next_version: page.next_version().value(),
        head_version: page.head_version().value(),
    })
}

/// One page of a named global feed.
pub fn pull_protocol_events_response_from(
    output: &PullProtocolEventsOutput,
) -> Result<pb::PullProtocolEventsResponse, DomainError> {
    Ok(pb::PullProtocolEventsResponse {
        records: output
            .records()
            .iter()
            .map(|positioned| {
                Ok(pb::PositionedProtocolEvent {
                    global_position: positioned.position.value(),
                    record: Some(protocol_event_record_from(&positioned.record)?),
                })
            })
            .collect::<Result<Vec<_>, DomainError>>()?,
        acknowledged_through: output
            .acknowledged_through()
            .map(made_core::value_objects::GlobalPosition::value),
    })
}

/// One sealed record.
pub(super) fn protocol_event_record_from(
    record: &AuditRecord,
) -> Result<pb::ProtocolEventRecord, DomainError> {
    let event = match record.event() {
        Some(event) => {
            // The very bytes the digest was taken over. Serializing
            // fails only where the engine's own types cannot be
            // written, which is not the caller's doing.
            let json = serde_json::to_value(event).map_err(|_| DomainError::InvariantViolated {
                reason: "a sealed protocol event cannot be rendered",
            })?;
            Some(
                struct_from_json(&json).ok_or(DomainError::InvariantViolated {
                    reason: "a sealed protocol event is not an object",
                })?,
            )
        }
        None => None,
    };

    Ok(pb::ProtocolEventRecord {
        event_id: record.event_id().as_str().to_owned(),
        event_type: record.event_type().as_str().to_owned(),
        schema_version: record.schema_version(),
        protocol_id: record.protocol_id().as_str().to_owned(),
        definition_name: record.definition_name().as_str().to_owned(),
        definition_version: record.definition_version().as_str().to_owned(),
        sequence: record.sequence().value(),
        occurred_at: record.occurred_at().format(&Rfc3339).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "an audit record timestamp cannot be rendered canonically",
            }
        })?,
        actor: Some(pb::ProtocolEventActor {
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
        authorization: record
            .authorization_evidence()
            .map(super::protocol_authorization::evidence_to_proto)
            .transpose()?,
    })
}

/// The ordered contributions of one session.
#[must_use]
pub fn get_protocol_transcript_response_from(
    transcript: &ProtocolTranscript,
) -> pb::GetProtocolTranscriptResponse {
    pb::GetProtocolTranscriptResponse {
        entries: transcript
            .contributions()
            .iter()
            .map(|contribution| pb::ProtocolTranscriptEntry {
                step_id: contribution.step_id().as_str().to_owned(),
                role_id: contribution.role_id().as_str().to_owned(),
                output: Some(attributes_to_struct(contribution.output().attributes())),
                state_visit: contribution.state_visit().get(),
                state_iteration: contribution.state_iteration().get(),
            })
            .collect(),
    }
}

/// The report and what it was projected from.
#[must_use]
pub fn generate_protocol_report_response_from(
    report: &ProtocolReport,
) -> pb::GenerateProtocolReportResponse {
    pb::GenerateProtocolReportResponse {
        report_markdown: report.markdown().to_owned(),
        protocol_ids: report
            .bindings()
            .iter()
            .map(|binding| binding.protocol_id().as_str().to_owned())
            .collect(),
        protocol_count: u32::try_from(report.protocol_count()).unwrap_or(u32::MAX),
        completed_count: u32::try_from(report.completed_count()).unwrap_or(u32::MAX),
        incomplete_count: u32::try_from(report.incomplete_count()).unwrap_or(u32::MAX),
        definition_bindings: report
            .bindings()
            .iter()
            .map(|binding| pb::ProtocolReportBinding {
                protocol_id: binding.protocol_id().as_str().to_owned(),
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
pub fn verify_protocol_journal_response_from(
    verdict: &ProtocolJournalVerdict,
) -> pb::VerifyProtocolJournalResponse {
    pb::VerifyProtocolJournalResponse {
        protocol_id: verdict.protocol_id().as_str().to_owned(),
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

    use made_core::entities::protocol_events::ProtocolInstanceStarted;
    use made_core::entities::{AuditFact, ProtocolEvent};
    use made_core::value_objects::{
        Attributes, AuditActor, AuditActorKind, AuditChainDefect, AuditChainVerdict, EventId,
        ProtocolContext, ProtocolId, ProtocolName, ProtocolStepContribution, ProtocolVersion,
        RoleId, StateId, StepId, StepOutput, StreamVersion,
    };
    use prost_types::value::Kind;
    use time::macros::datetime;
    use time::OffsetDateTime;

    use super::*;

    const AT: OffsetDateTime = datetime!(2026-09-16 09:00:00 UTC);

    fn record() -> AuditRecord {
        let protocol_id = ProtocolId::new("session-1").unwrap();
        AuditRecord::first(AuditFact {
            event_id: EventId::new("e1").unwrap(),
            event: ProtocolEvent::ProtocolInstanceStarted(ProtocolInstanceStarted {
                protocol_id: protocol_id.clone(),
                definition_name: ProtocolName::new("planning_protocol").unwrap(),
                definition_version: ProtocolVersion::v1(),
                initial_state: StateId::new("OPEN").unwrap(),
                step_ids: BTreeSet::from([StepId::new("draft").unwrap()]),
                context: ProtocolContext::empty(),
                bound_definition: None,
                lineage: None,
                succession: None,
                budget_account_id: None,
                protocol_deadline: None,
                state_deadline: None,
                created_at: AT,
            }),
            protocol_id,
            definition_name: ProtocolName::new("planning_protocol").unwrap(),
            definition_version: ProtocolVersion::v1(),
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
        let wire = protocol_event_record_from(&record).unwrap();

        assert_eq!(wire.event_id, "e1");
        assert_eq!(wire.event_type, "protocol_instance_started");
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
            Some(Kind::StringValue("protocol_instance_started".to_owned()))
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
        let transcript = ProtocolTranscript::new(vec![ProtocolStepContribution::new(
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

        let wire = get_protocol_transcript_response_from(&transcript);

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
        let verdict = ProtocolJournalVerdict::new(
            ProtocolId::new("session-1").unwrap(),
            StreamVersion::new(4),
            4,
            AuditChainVerdict::Intact,
        );

        let wire = verify_protocol_journal_response_from(&verdict);

        assert_eq!(wire.protocol_id, "session-1");
        assert_eq!(wire.head_version, 4);
        assert_eq!(wire.record_count, 4);
        assert!(wire.intact);
        assert_eq!(wire.first_broken_sequence, 0);
        assert!(wire.reason.is_empty());
    }

    #[test]
    fn a_broken_journal_carries_the_first_position_and_why() {
        let verdict = ProtocolJournalVerdict::new(
            ProtocolId::new("session-1").unwrap(),
            StreamVersion::new(4),
            3,
            AuditChainVerdict::Broken(AuditChainDefect::DigestAltered {
                at: AuditSequence::new(2).unwrap(),
            }),
        );

        let wire = verify_protocol_journal_response_from(&verdict);

        assert!(!wire.intact);
        assert_eq!(wire.first_broken_sequence, 2);
        assert!(wire.reason.contains("digest"), "{}", wire.reason);
        assert_eq!(wire.record_count, 3);
    }
}
