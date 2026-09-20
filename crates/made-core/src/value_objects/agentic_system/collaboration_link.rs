use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

use super::{ChannelName, CollaborationKind, ParticipantId};

/// One arrow of the topology: who works with whom, and in what way.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
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

/// Decoding goes through the constructor, so a stored or transmitted
/// link joining a participant to itself is refused where it arrives
/// rather than drawn as a loop nobody meant.
///
/// The wire shape lives inside the function because it is this
/// function's business and nothing else's.
impl<'de> Deserialize<'de> for CollaborationLink {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            from: ParticipantId,
            to: ParticipantId,
            kind: CollaborationKind,
            #[serde(default)]
            channel: Option<ChannelName>,
            #[serde(default)]
            handoff: bool,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.from, wire.to, wire.kind, wire.channel, wire.handoff)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn participant(raw: &str) -> ParticipantId {
        ParticipantId::new(raw).unwrap()
    }

    #[test]
    fn a_stored_self_link_is_refused_on_the_way_in() {
        let refused = serde_json::from_str::<CollaborationLink>(
            r#"{"from":"a","to":"a","kind":"communication"}"#,
        );
        let accepted = serde_json::from_str::<CollaborationLink>(
            r#"{"from":"a","to":"b","kind":"communication"}"#,
        );

        assert!(refused.is_err());
        assert!(accepted.is_ok());
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
