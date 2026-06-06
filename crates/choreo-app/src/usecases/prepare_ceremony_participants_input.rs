use super::CeremonyParticipantDescriptor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareCeremonyParticipantsInput {
    participants: Vec<CeremonyParticipantDescriptor>,
}

impl PrepareCeremonyParticipantsInput {
    #[must_use]
    pub fn new(participants: Vec<CeremonyParticipantDescriptor>) -> Self {
        Self { participants }
    }

    #[must_use]
    pub fn participants(&self) -> &[CeremonyParticipantDescriptor] {
        &self.participants
    }

    #[must_use]
    pub fn into_participants(self) -> Vec<CeremonyParticipantDescriptor> {
        self.participants
    }
}
