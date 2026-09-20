//! What a session left behind, as the in-process arm answers it.
//!
//! The records go out as their own serde form — the very shape the
//! event store holds and the digest covers — so a client can read them
//! back into `AuditRecord` and verify the chain itself. The gRPC arm
//! rebuilds exactly this JSON from the proto record, and the parity
//! session compares the two field for field.

use futures::StreamExt;
use made_app::usecases::{
    CeremonyEventPage, CeremonyJournalVerdict, CeremonyProgressFrame, CeremonyProgressStream,
    CeremonyReport, PullCeremonyEventsOutput,
};
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

pub(super) async fn collect_ceremony_progress(
    mut stream: CeremonyProgressStream,
) -> Result<Value, ToolError> {
    let mut records = Vec::new();
    let mut agent_snapshot = None;
    let mut agent_activity = Vec::new();
    let mut ending = None;
    while let Some(item) = stream.next().await {
        match item? {
            CeremonyProgressFrame::Record(record) if ending.is_none() => {
                records.push(audit_record_view(&record)?.to_json());
            }
            CeremonyProgressFrame::AgentSnapshot(snapshot) if ending.is_none() => {
                agent_snapshot = Some(json!({
                    "agents": snapshot.agents().iter().map(agent_status_view).collect::<Result<Vec<_>, _>>()?,
                    "activity_head_sequence": snapshot.activity_head_sequence(),
                    "complete": snapshot.is_complete(),
                }));
            }
            CeremonyProgressFrame::AgentActivity(activity) if ending.is_none() => {
                agent_activity.push(json!({
                    "sequence": activity.sequence(),
                    "kind": activity.kind().as_str(),
                    "source": "host_assertion",
                    "status": agent_status_view(activity.status())?,
                }));
            }
            CeremonyProgressFrame::End(end) if ending.is_none() => ending = Some(end),
            _ => {
                return Err(ToolError::refused(
                    "progress stream emitted a frame after its end",
                ))
            }
        }
    }
    let end = ending.ok_or_else(|| {
        ToolError::refused("progress stream closed without its required end frame")
    })?;
    Ok(json!({
        "records": records,
        "agent_snapshot": agent_snapshot,
        "agent_activity": agent_activity,
        "resume_after_sequence": end.resume_after_sequence().value(),
        "head_sequence": end.head_sequence().value(),
        "resume_after_activity_sequence": end.resume_after_activity_sequence(),
        "activity_head_sequence": end.activity_head_sequence(),
        "end_reason": end.reason().as_str(),
    }))
}

fn agent_status_view(
    status: &made_core::entities::CeremonyAgentStatus,
) -> Result<Value, ToolError> {
    let observed_at = status.observed_at().format(&Rfc3339).map_err(|error| {
        ToolError::refused(format!(
            "agent status timestamp cannot be rendered: {error}"
        ))
    })?;
    Ok(json!({
        "ceremony_id": status.ceremony_id().as_str(),
        "agent_execution_id": status.agent_execution_id().as_str(),
        "operation_id": status.operation_id().as_str(),
        "claim_owner_id": status.claim_owner_id().as_str(),
        "logical_worker_id": status.logical_worker_id().as_str(),
        "host_agent_id": status.host_agent_id().as_str(),
        "host_agent_incarnation": status.host_agent_incarnation().as_str(),
        "previous_host_agent_id": status.previous_host_agent_id().map(made_core::value_objects::LeaseOwnerId::as_str),
        "previous_host_agent_incarnation": status.previous_host_agent_incarnation().map(made_core::value_objects::HostAgentIncarnation::as_str),
        "role_id": status.role_id().as_str(), "step_id": status.step_id().as_str(),
        "attempt": status.attempt(), "execution_status": status.execution_status(),
        "liveness": status.liveness(), "source": status.source(),
        "requested_model": status.requested_model(),
        "requested_reasoning_effort": status.requested_reasoning_effort(),
        "actual_model": status.actual_model(),
        "actual_reasoning_effort": status.actual_reasoning_effort(),
        "activity": status.activity(), "blocker": status.blocker(),
        "dependency": status.dependency(), "task_summary": status.task_summary(),
        "evidence_references": status.evidence_references(),
        "usage_kind": status.usage_kind(), "usage_value": status.usage_value(),
        "observed_at": observed_at, "report_sequence": status.report_sequence(),
        "idempotency_key": status.idempotency_key(), "claim_fence": status.claim_fence().as_str(),
    }))
}

pub(super) fn present_pulled_ceremony_events(
    output: &PullCeremonyEventsOutput,
) -> Result<Value, ToolError> {
    let records = output
        .records()
        .iter()
        .map(|positioned| {
            let mut view = audit_record_view(&positioned.record)?;
            view.global_position = Some(positioned.position.value());
            Ok(view.to_json())
        })
        .collect::<Result<Vec<_>, ToolError>>()?;
    Ok(json!({
        "records": records,
        "acknowledged_through": output.acknowledged_through().map(made_core::value_objects::GlobalPosition::value),
    }))
}

pub(super) fn audit_record_view(record: &AuditRecord) -> Result<AuditRecordView, ToolError> {
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
        global_position: None,
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
        authorization: record
            .authorization_evidence()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| {
                ToolError::refused(format!("sealed authorization cannot be rendered: {error}"))
            })?,
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
                "state_iteration": contribution.state_iteration().get(),
                "state_visit": contribution.state_visit().get(),
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
