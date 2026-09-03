use made_core::ports::AgentDescriptor;
use made_core::value_objects::{AgentId, AgentKind, Specialty};
use made_proto::v1 as pb;

use super::descriptor_error::DescriptorError;

/// Map the proto request into a domain descriptor.
///
/// Precedence on specialty: the dedicated top-level `specialty` field
/// on the request wins when non-empty; otherwise the nested
/// `agent.specialty` is used. This keeps the proto backwards-
/// compatible without encoding two sources of truth downstream.
pub(super) fn descriptor_from_register_request(
    req: pb::RegisterAgentRequest,
) -> Result<AgentDescriptor, DescriptorError> {
    let summary = req.agent.ok_or(DescriptorError::MissingAgentSummary)?;
    let specialty_str = if req.specialty.trim().is_empty() {
        summary.specialty
    } else {
        req.specialty
    };
    Ok(AgentDescriptor {
        id: AgentId::new(summary.agent_id)?,
        specialty: Specialty::new(specialty_str)?,
        kind: AgentKind::new(summary.kind)?,
        attributes: crate::grpc::mappers::attributes_from_struct(req.agent_config)?,
    })
}
