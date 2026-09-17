//! What a session left behind, as the in-process arm answers it.
//!
//! The records go out as their own serde form — the very shape the
//! event store holds and the digest covers — so a client can read them
//! back into `AuditRecord` and verify the chain itself. The gRPC arm
//! rebuilds exactly this JSON from the proto record, and the parity
//! session compares the two field for field.

use made_app::usecases::{CeremonyEventPage, CeremonyJournalVerdict, CeremonyReport};
use made_core::entities::AuditRecord;
use made_core::value_objects::CeremonyTranscript;
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;

use crate::protocol::{CeremonyJournalVerdictView, ToolError, REPORT_IS_PERSISTED};
use crate::renderers::{AuditRecordView, CeremonyEventPageView};

/// One page of a stream.
pub(super) fn present_ceremony_events(page: &CeremonyEventPage) -> Result<Value, ToolError> {
    let records = page
        .records()
        .iter()
        .map(audit_record_view)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CeremonyEventPageView::new(
        records,
        page.next_version().value(),
        page.head_version().value(),
    )
    .to_json())
}

fn audit_record_view(record: &AuditRecord) -> Result<AuditRecordView, ToolError> {
    let actor = serde_json::to_value(record.actor()).map_err(|error| {
        ToolError::refused(format!(
            "a sealed ceremony actor cannot be rendered: {error}"
        ))
    })?;
    let event = record
        .event()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| {
            ToolError::refused(format!(
                "a sealed ceremony event cannot be rendered: {error}"
            ))
        })?;
    let occurred_at = record.occurred_at().format(&Rfc3339).map_err(|error| {
        ToolError::refused(format!(
            "a sealed ceremony record timestamp cannot be rendered: {error}"
        ))
    })?;

    Ok(AuditRecordView {
        event_id: record.event_id().as_str().to_owned(),
        event_type: record.event_type().as_str().to_owned(),
        schema_version: record.schema_version(),
        ceremony_id: record.ceremony_id().as_str().to_owned(),
        definition_name: record.definition_name().as_str().to_owned(),
        definition_version: record.definition_version().as_str().to_owned(),
        sequence: record.sequence().value(),
        occurred_at,
        actor,
        correlation_id: record.correlation_id().map(|id| id.as_str().to_owned()),
        causation_id: record.causation_id().map(|id| id.as_str().to_owned()),
        trace_id: record.trace_id().map(str::to_owned),
        event_schema_version: record
            .event_schema_version()
            .map(made_core::value_objects::EventSchemaVersion::get),
        event,
        previous_record_hash: record
            .previous_record_hash()
            .map(|hash| hash.as_bytes().to_vec()),
        record_hash: record.record_hash().as_bytes().to_vec(),
    })
}

/// The verdict on one session's chain.
///
/// Filled into the view both arms render through, so the in-process
/// answer and the one that came back over gRPC are the same JSON by
/// construction rather than by two people writing the same keys.
#[must_use]
pub(super) fn present_ceremony_journal_verdict(verdict: &CeremonyJournalVerdict) -> Value {
    CeremonyJournalVerdictView {
        ceremony_id: verdict.ceremony_id().as_str().to_owned(),
        head_version: verdict.head_version().value(),
        record_count: verdict.record_count() as u64,
        intact: verdict.is_intact(),
        first_broken_sequence: verdict
            .first_broken_sequence()
            .map(made_core::value_objects::AuditSequence::value),
        reason: verdict.reason(),
    }
    .to_json()
}

/// The ordered contributions of one session.
#[must_use]
pub(super) fn present_ceremony_transcript(transcript: &CeremonyTranscript) -> Value {
    json!({
        "entries": transcript
            .contributions()
            .iter()
            .map(|contribution| json!({
                "step_id": contribution.step_id().as_str(),
                "role_id": contribution.role_id().as_str(),
                "output": contribution.output().attributes().as_map(),
            }))
            .collect::<Vec<_>>(),
        "entry_count": transcript.len(),
    })
}

/// The report, exactly as this tool has always answered it.
#[must_use]
pub(super) fn present_ceremony_report(report: &CeremonyReport) -> Value {
    json!({
        "report_markdown": report.markdown(),
        "ceremony_ids": report
            .bindings()
            .iter()
            .map(|binding| binding.ceremony_id().as_str())
            .collect::<Vec<_>>(),
        "ceremony_count": report.ceremony_count(),
        "completed_count": report.completed_count(),
        "incomplete_count": report.incomplete_count(),
        "definition_bindings": report
            .bindings()
            .iter()
            .map(|binding| json!({
                "ceremony_id": binding.ceremony_id().as_str(),
                "definition_name": binding.definition_name().as_str(),
                "definition_version": binding.definition_version().as_str(),
                "definition_digest": binding.definition_digest().to_hex(),
                "bound_definition_digest": binding
                    .bound_definition_digest()
                    .map(|digest| digest.to_hex()),
            }))
            .collect::<Vec<_>>(),
        "persisted": REPORT_IS_PERSISTED,
    })
}
