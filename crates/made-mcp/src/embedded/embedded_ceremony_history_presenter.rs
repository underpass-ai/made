//! What a session left behind, as the in-process arm answers it.
//!
//! The records go out as their own serde form — the very shape the
//! event store holds and the digest covers — so a client can read them
//! back into `AuditRecord` and verify the chain itself. The gRPC arm
//! rebuilds exactly this JSON from the proto record, and the parity
//! session compares the two field for field.

use made_app::usecases::{CeremonyEventPage, CeremonyReport};
use made_core::value_objects::CeremonyTranscript;
use serde_json::{json, Value};

use crate::protocol::{ToolError, REPORT_IS_PERSISTED};

/// One page of a stream.
pub(super) fn present_ceremony_events(page: &CeremonyEventPage) -> Result<Value, ToolError> {
    // Serializing a sealed record fails only where the engine's own
    // types cannot be written, which is not the caller's doing.
    let records = serde_json::to_value(page.records()).map_err(|error| {
        ToolError::refused(format!(
            "a sealed ceremony record cannot be rendered: {error}"
        ))
    })?;
    Ok(json!({
        "records": records,
        "record_count": page.records().len(),
        "next_version": page.next_version().value(),
        "head_version": page.head_version().value(),
        "has_more": page.has_more(),
    }))
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
