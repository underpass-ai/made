use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::{ChannelName, CollaborationKind, ParticipantId};

/// One arrow of the topology: who works with whom, and in what way.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CollaborationLink {
    from: ParticipantId,
    to: ParticipantId,
    kind: CollaborationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    channel: Option<ChannelName>,
    #[serde(default)]
    handoff: bool,
}

impl CollaborationLink {
    /// Construct a link between two distinct participants.
    ///
    /// A participant collaborating with itself is refused here rather
    /// than reported as a finding: it says nothing about the system,
    /// and a diagram would draw a loop nobody meant.
    pub fn new(
        from: ParticipantId,
        to: ParticipantId,
        kind: CollaborationKind,
        channel: Option<ChannelName>,
        handoff: bool,
    ) -> Result<Self, DomainError> {
        if from == to {
            return Err(DomainError::InvariantViolated {
                reason: "a collaboration link cannot join a participant to itself",
            });
        }
        Ok(Self {
            from,
            to,
            kind,
            channel,
            handoff,
        })
    }

    #[must_use]
    pub const fn from(&self) -> &ParticipantId {
        &self.from
    }

    #[must_use]
    pub const fn to(&self) -> &ParticipantId {
        &self.to
    }

    #[must_use]
    pub const fn kind(&self) -> CollaborationKind {
        self.kind
    }

    #[must_use]
    pub const fn channel(&self) -> Option<&ChannelName> {
        self.channel.as_ref()
    }

    /// Whether the work itself changes hands along this link.
    #[must_use]
    pub const fn is_handoff(&self) -> bool {
        self.handoff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn participant(raw: &str) -> ParticipantId {
        ParticipantId::new(raw).unwrap()
    }

    #[test]
    fn a_participant_cannot_collaborate_with_itself() {
        assert!(CollaborationLink::new(
            participant("reviewer"),
            participant("reviewer"),
            CollaborationKind::Communication,
            None,
            false,
        )
        .is_err());
    }
}
