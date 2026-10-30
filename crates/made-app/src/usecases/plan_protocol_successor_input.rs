use made_core::value_objects::{ProtocolId, ProtocolName, ProtocolVersion};

/// Which protocol would hand off, and to which published definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanProtocolSuccessorInput {
    pub instance_id: ProtocolId,
    pub definition_name: ProtocolName,
    pub definition_version: ProtocolVersion,
}
