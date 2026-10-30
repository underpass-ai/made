use made_core::value_objects::{HostHandoffDeclaration, ProtocolId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordProtocolHostHandoffInput {
    pub protocol_id: ProtocolId,
    pub declaration: HostHandoffDeclaration,
}
