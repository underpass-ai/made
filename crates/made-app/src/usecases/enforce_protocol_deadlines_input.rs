use made_core::value_objects::ProtocolId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforceProtocolDeadlinesInput {
    pub(crate) instance_id: ProtocolId,
}

impl EnforceProtocolDeadlinesInput {
    #[must_use]
    pub fn new(instance_id: ProtocolId) -> Self {
        Self { instance_id }
    }

    #[must_use]
    pub const fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }
}
