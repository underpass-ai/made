use made_app::usecases::{DesignedProtocol, ProtocolDraftView};
use made_core::value_objects::{ProtocolDefinitionDiff, ProtocolValidationFinding};
use serde_json::{json, Value};

/// Render a draft analysis for machine and human consumers.
///
/// The domain report is deliberately not serializable, so mapping it to
/// a wire shape happens here, at the adapter boundary. What the answer
/// *is* — the counts and the explanation — comes from
/// [`ProtocolDraftView`], so an author gets the same answer whichever
/// distribution they asked.
pub(crate) struct EmbeddedProtocolDraftPresenter;

impl EmbeddedProtocolDraftPresenter {
    pub(crate) fn present_design(
        definition_yaml: &str,
        view: &ProtocolDraftView<'_>,
        designed: &DesignedProtocol,
    ) -> Value {
        json!({
            "protocol": view.name().as_str(),
            "version": view.version().as_str(),
            "definition_yaml": definition_yaml,
            "publishable": view.is_publishable(),
            "design": {
                "topology": designed.topology(),
                "stages": designed.stage_count(),
                "participants": designed.participant_count(),
                "final_approval_required": designed.final_approval_required(),
            },
            "analysis": Self::present_validation(view),
            "published": false,
            "started": false,
        })
    }

    pub(crate) fn present_validation(view: &ProtocolDraftView<'_>) -> Value {
        json!({
            "protocol": view.name().as_str(),
            "version": view.version().as_str(),
            "publishable": view.is_publishable(),
            "error_count": view.error_count(),
            "warning_count": view.warning_count(),
            "findings": view.findings().iter().map(present_finding).collect::<Vec<_>>(),
        })
    }

    pub(crate) fn present_explanation(view: &ProtocolDraftView<'_>) -> Value {
        let summary = view.summary();
        json!({
            "protocol": view.name().as_str(),
            "version": view.version().as_str(),
            "publishable": view.is_publishable(),
            "summary": {
                "states": summary.states(),
                "initial_states": summary.initial_states(),
                "terminal_states": summary.terminal_states(),
                "transitions": summary.transitions(),
                "steps": summary.steps(),
                "guards": summary.guards(),
                "roles": summary.roles(),
            },
            "narrative": view.narrative(),
        })
    }
}

fn present_finding(finding: &ProtocolValidationFinding) -> Value {
    json!({
        "severity": if finding.is_blocking() { "error" } else { "warning" },
        "locus": serde_json::to_value(finding.locus()).unwrap_or(Value::Null),
        "message": finding.defect().to_string(),
    })
}

/// What changed between two definitions, and for each change whether a
/// running session could go on. Rendered here and by the gRPC adapter
/// from the same domain diff, so an author gets one answer.
pub(crate) fn present_definition_diff(diff: &ProtocolDefinitionDiff) -> Value {
    json!({
        "identical": diff.is_identical(),
        "strands_running_sessions": diff.strands_running_sessions(),
        "strand_count": diff.strand_count(),
        "changes": diff
            .changes()
            .iter()
            .map(|change| json!({
                "kind": change.kind().as_label(),
                "locus": serde_json::to_value(change.locus()).unwrap_or(Value::Null),
                "impact": change.impact().as_label(),
                "detail": change.detail(),
            }))
            .collect::<Vec<_>>(),
    })
}
