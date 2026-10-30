//! Authoring a protocol: application → proto.
//!
//! What a draft is and what is wrong with it comes from
//! [`ProtocolDraftView`], the same projection the in-process surface
//! renders. Only the encoding is decided here — which is why the
//! locus travels as a `Struct` rather than as a string: the wire
//! carries the same shape the in-process JSON does, not a rendering
//! of it that a client would have to parse a second time.

use made_app::usecases::{ProtocolDefinitionSource, ProtocolDraftView};
use made_core::entities::PublicationOutcome;
use made_core::error::DomainError;
use made_core::value_objects::{
    ProtocolDefinitionDiff, ProtocolName, ProtocolValidationFinding, ProtocolVersion,
};
use made_proto::v1 as pb;

use crate::yaml::ProtocolDefinitionYaml;

use super::attributes::struct_from_json;

pub fn validate_protocol_draft_response_from(
    view: &ProtocolDraftView<'_>,
) -> pb::ValidateProtocolDraftResponse {
    pb::ValidateProtocolDraftResponse {
        protocol: view.name().as_str().to_owned(),
        version: view.version().as_str().to_owned(),
        publishable: view.is_publishable(),
        error_count: u32::try_from(view.error_count()).unwrap_or(u32::MAX),
        warning_count: u32::try_from(view.warning_count()).unwrap_or(u32::MAX),
        findings: view.findings().iter().map(finding_from).collect(),
    }
}

pub fn explain_protocol_draft_response_from(
    view: &ProtocolDraftView<'_>,
) -> pb::ExplainProtocolDraftResponse {
    let summary = view.summary();
    pb::ExplainProtocolDraftResponse {
        protocol: view.name().as_str().to_owned(),
        version: view.version().as_str().to_owned(),
        publishable: view.is_publishable(),
        summary: Some(pb::ProtocolDraftSummary {
            states: count(summary.states()),
            initial_states: count(summary.initial_states()),
            terminal_states: count(summary.terminal_states()),
            transitions: count(summary.transitions()),
            steps: count(summary.steps()),
            guards: count(summary.guards()),
            roles: count(summary.roles()),
        }),
        narrative: view.narrative().to_vec(),
    }
}

pub fn publish_protocol_definition_response_from(
    outcome: &PublicationOutcome,
) -> pb::PublishProtocolDefinitionResponse {
    match outcome {
        PublicationOutcome::Published(published) => pb::PublishProtocolDefinitionResponse {
            outcome: "published".to_owned(),
            protocol: published.name().as_str().to_owned(),
            version: published.version().as_str().to_owned(),
            digest: published.digest().to_hex(),
            ..pb::PublishProtocolDefinitionResponse::default()
        },
        PublicationOutcome::AlreadyPublished(published) => pb::PublishProtocolDefinitionResponse {
            outcome: "already_published".to_owned(),
            protocol: published.name().as_str().to_owned(),
            version: published.version().as_str().to_owned(),
            digest: published.digest().to_hex(),
            ..pb::PublishProtocolDefinitionResponse::default()
        },
        // Not an error: offering the same content under an occupied
        // version is idempotent, and offering different content is a
        // refusal the caller can act on, which is why both digests
        // come back rather than a message saying they differ.
        PublicationOutcome::VersionOccupied { published, offered } => {
            pb::PublishProtocolDefinitionResponse {
                outcome: "version_occupied".to_owned(),
                published_digest: published.to_hex(),
                offered_digest: offered.to_hex(),
                ..pb::PublishProtocolDefinitionResponse::default()
            }
        }
    }
}

pub(super) fn finding_from(finding: &ProtocolValidationFinding) -> pb::ProtocolDraftFinding {
    pb::ProtocolDraftFinding {
        severity: if finding.is_blocking() {
            "error".to_owned()
        } else {
            "warning".to_owned()
        },
        locus: serde_json::to_value(finding.locus())
            .ok()
            .as_ref()
            .and_then(struct_from_json),
        message: finding.defect().to_string(),
    }
}

pub(super) fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// One side of a comparison. Naming a published version and supplying
/// a document are the two things a caller can mean; asking for both,
/// or neither, is not a third thing with a sensible reading.
pub fn protocol_definition_source_from_proto(
    reference: Option<pb::ProtocolDefinitionRef>,
    side: &'static str,
) -> Result<ProtocolDefinitionSource, DomainError> {
    let reference = reference.ok_or(DomainError::EmptyField { field: side })?;
    let named = !reference.protocol.trim().is_empty() || !reference.version.trim().is_empty();
    let supplied = !reference.definition_yaml.trim().is_empty();

    match (named, supplied) {
        (true, false) => Ok(ProtocolDefinitionSource::published(
            ProtocolName::new(reference.protocol)?,
            ProtocolVersion::new(reference.version)?,
        )),
        (false, true) => Ok(ProtocolDefinitionSource::supplied(
            ProtocolDefinitionYaml::parse_str(&reference.definition_yaml)?,
        )),
        (true, true) => Err(DomainError::InvariantViolated {
            reason: "a definition is either published or supplied, not both",
        }),
        (false, false) => Err(DomainError::EmptyField { field: side }),
    }
}

pub fn diff_protocol_definitions_response_from(
    diff: &ProtocolDefinitionDiff,
) -> pb::DiffProtocolDefinitionsResponse {
    pb::DiffProtocolDefinitionsResponse {
        identical: diff.is_identical(),
        strands_running_sessions: diff.strands_running_sessions(),
        strand_count: u32::try_from(diff.strand_count()).unwrap_or(u32::MAX),
        changes: diff
            .changes()
            .iter()
            .map(|change| pb::ProtocolDefinitionChange {
                kind: change.kind().as_label().to_owned(),
                locus: serde_json::to_value(change.locus())
                    .ok()
                    .as_ref()
                    .and_then(struct_from_json),
                impact: change.impact().as_label().to_owned(),
                detail: change.detail().to_owned(),
            })
            .collect(),
    }
}
