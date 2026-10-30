use super::ProtocolParticipantDescriptor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareProtocolParticipantsInput {
    participants: Vec<ProtocolParticipantDescriptor>,
}

impl PrepareProtocolParticipantsInput {
    #[must_use]
    pub fn new(participants: Vec<ProtocolParticipantDescriptor>) -> Self {
        Self { participants }
    }

    #[must_use]
    pub fn participants(&self) -> &[ProtocolParticipantDescriptor] {
        &self.participants
    }

    #[must_use]
    pub fn into_participants(self) -> Vec<ProtocolParticipantDescriptor> {
        self.participants
    }
}
