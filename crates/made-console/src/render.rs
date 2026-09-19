use made_client::v1::{ArtifactRecord, CeremonyInstanceState};
use made_client::{CeremonyTree, CeremonyTreeNode, ProgressBatch};
use serde_json::{json, Value};

use crate::OutputFormat;

pub fn instance(instance: &CeremonyInstanceState, format: OutputFormat) -> String {
    render(&instance_value(instance), format)
}

pub fn instances(instances: &[CeremonyInstanceState], format: OutputFormat) -> String {
    let values: Vec<_> = instances.iter().map(instance_value).collect();
    render(&json!({ "ceremonies": values }), format)
}

pub fn tree(tree: &CeremonyTree, format: OutputFormat) -> String {
    let roots: Vec<_> = tree.roots().iter().map(tree_node_value).collect();
    render(&json!({ "roots": roots }), format)
}

pub fn artifacts(
    artifacts: &[ArtifactRecord],
    next_cursor: Option<&str>,
    format: OutputFormat,
) -> String {
    let values: Vec<_> = artifacts
        .iter()
        .map(|record| {
            let reference = record.artifact.as_ref();
            json!({
                "artifact_id": reference.map(|value| value.artifact_id.as_str()),
                "digest": reference.map(|value| value.digest.as_str()),
                "size_bytes": reference.map(|value| value.size_bytes),
                "media_type": reference.map(|value| value.media_type.as_str()),
                "source_kind": reference
                    .and_then(|value| value.provenance.as_ref())
                    .map(|value| value.source_kind),
                "tombstoned": record.tombstone.is_some(),
            })
        })
        .collect();
    render(
        &json!({ "artifacts": values, "next_cursor": next_cursor }),
        format,
    )
}

pub fn progress(batch: &ProgressBatch, format: OutputFormat) -> String {
    let records: Vec<_> = batch
        .records()
        .iter()
        .map(|record| {
            json!({
                "event_id": record.event_id,
                "event_type": record.event_type,
                "sequence": record.sequence,
                "occurred_at": record.occurred_at,
                "late": record.event_type == "late_step_result_observed",
            })
        })
        .collect();
    render(
        &json!({
            "records": records,
            "resume_after_sequence": batch.checkpoint().after_sequence(),
            "head_sequence": batch.head_sequence(),
            "end_reason": batch.end_reason().as_str_name(),
        }),
        format,
    )
}

pub fn message(value: &Value, format: OutputFormat) -> String {
    render(value, format)
}

fn instance_value(instance: &CeremonyInstanceState) -> Value {
    let steps: Vec<_> = instance
        .steps
        .iter()
        .map(|step| {
            json!({
                "step_id": step.step_id,
                "state_id": step.state_id,
                "status": step.status,
                "attempt": step.attempt,
                "state_visit": step.state_visit,
                "state_iteration": step.state_iteration,
                "step_iteration": step.iteration,
                "error": optional_text(&step.error),
            })
        })
        .collect();
    json!({
        "ceremony_id": instance.ceremony_id,
        "definition": {
            "name": instance.definition_name,
            "version": instance.definition_version,
        },
        "lifecycle": instance.lifecycle,
        "end_reason": optional_text(&instance.end_reason),
        "completed": instance.completed,
        "current_state": instance.current_state,
        "current_state_visit": instance.current_state_visit,
        "current_state_iteration": instance.current_state_iteration,
        "next_step_id": optional_text(&instance.next_step_id),
        "claimable_step_ids": instance.claimable_step_ids,
        "waiting_for_human": instance.waiting_for_human,
        "open_intervention_ids": instance.open_intervention_ids,
        "paused_at": optional_text(&instance.paused_at),
        "ended_at": optional_text(&instance.ended_at),
        "ceremony_deadline_at": optional_text(&instance.ceremony_deadline_at),
        "state_deadline_at": optional_text(&instance.state_deadline_at),
        "rehydratable": instance.rehydratable,
        "unrehydratable_reason": optional_text(&instance.unrehydratable_reason),
        "budget": "unknown",
        "steps": steps,
    })
}

fn tree_node_value(node: &CeremonyTreeNode) -> Value {
    let children: Vec<_> = node.children().iter().map(tree_node_value).collect();
    json!({
        "ceremony": instance_value(node.instance()),
        "children": children,
    })
}

fn optional_text(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn render(value: &Value, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => serde_json::to_string_pretty(value).expect("JSON values serialize"),
        OutputFormat::Json => serde_json::to_string(value).expect("JSON values serialize"),
    }
}
