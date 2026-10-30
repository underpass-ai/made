use super::{json, pb, pb_struct_to_json, Value};

pub(crate) fn design_protocol_to_json(response: pb::DesignProtocolResponse) -> Value {
    let design = response.design.unwrap_or_default();
    json!({
        "protocol": response.protocol,
        "version": response.version,
        "definition_yaml": response.definition_yaml,
        "publishable": response.publishable,
        "design": {
            "topology": design.topology,
            "stages": design.stages,
            "participants": design.participants,
            "final_approval_required": design.final_approval_required,
        },
        "analysis": {
            "protocol": response.protocol,
            "version": response.version,
            "publishable": response.publishable,
            "error_count": response.error_count,
            "warning_count": response.warning_count,
            "findings": response
                .findings
                .into_iter()
                .map(finding_to_json)
                .collect::<Vec<_>>(),
        },
        "published": false,
        "started": false,
    })
}

pub(crate) fn validate_protocol_draft_to_json(
    response: pb::ValidateProtocolDraftResponse,
) -> Value {
    json!({
        "protocol": response.protocol,
        "version": response.version,
        "publishable": response.publishable,
        "error_count": response.error_count,
        "warning_count": response.warning_count,
        "findings": response
            .findings
            .into_iter()
            .map(finding_to_json)
            .collect::<Vec<_>>(),
    })
}

/// One finding, rendered once. Validating a draft and designing one
/// answer with the same findings, and a reader that had to tell two
/// renderings apart would be reading the difference between the calls
/// rather than between the drafts.
fn finding_to_json(finding: pb::ProtocolDraftFinding) -> Value {
    json!({
        "severity": finding.severity,
        "locus": finding.locus.map_or(Value::Null, |locus| Value::Object(pb_struct_to_json(locus))),
        "message": finding.message,
    })
}

pub(crate) fn explain_protocol_draft_to_json(response: &pb::ExplainProtocolDraftResponse) -> Value {
    json!({
        "protocol": response.protocol,
        "version": response.version,
        "publishable": response.publishable,
        "summary": response.summary.as_ref().map_or_else(
            || json!({}),
            |summary| json!({
                "states": summary.states,
                "initial_states": summary.initial_states,
                "terminal_states": summary.terminal_states,
                "transitions": summary.transitions,
                "steps": summary.steps,
                "guards": summary.guards,
                "roles": summary.roles,
            }),
        ),
        "narrative": response.narrative.clone(),
    })
}

/// Publishing answers one of three outcomes, and each carries only the
/// fields that mean anything for it. Emitting the others as empty
/// strings would tell a reader that a refused publication has a digest
/// of "".
pub(crate) fn publish_protocol_definition_to_json(
    response: &pb::PublishProtocolDefinitionResponse,
) -> Value {
    if response.outcome == "version_occupied" {
        json!({
            "outcome": response.outcome,
            "published_digest": response.published_digest,
            "offered_digest": response.offered_digest,
        })
    } else {
        json!({
            "outcome": response.outcome,
            "protocol": response.protocol,
            "version": response.version,
            "digest": response.digest,
        })
    }
}

pub(crate) fn diff_protocol_definitions_to_json(
    response: pb::DiffProtocolDefinitionsResponse,
) -> Value {
    json!({
        "identical": response.identical,
        "strands_running_sessions": response.strands_running_sessions,
        "strand_count": response.strand_count,
        "changes": response
            .changes
            .into_iter()
            .map(|change| json!({
                "kind": change.kind,
                "locus": change.locus.map_or(Value::Null, |locus| Value::Object(pb_struct_to_json(locus))),
                "impact": change.impact,
                "detail": change.detail,
            }))
            .collect::<Vec<_>>(),
    })
}
