use made_core::value_objects::{CeremonyId, HostHandoffDeclaration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCeremonyHostHandoffInput {
    pub ceremony_id: CeremonyId,
    pub declaration: HostHandoffDeclaration,
}
