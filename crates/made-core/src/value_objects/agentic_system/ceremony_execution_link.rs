use serde::{Deserialize, Serialize};

use crate::value_objects::{CeremonyId, DefinitionPin};

use super::{LinkStatus, LoopRound, UnavailabilityReason};

/// What became of one composed ceremony inside one run.
///
/// The pin travels with the link rather than being looked up again, so
/// a run inspected long afterwards still says which bytes it started,
/// even if the design has moved on since.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyExecutionLink {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instance_id: Option<CeremonyId>,
    pin: DefinitionPin,
    status: LinkStatus,
    #[serde(default)]
    round: LoopRound,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    skipped_because: Option<UnavailabilityReason>,
}

impl CeremonyExecutionLink {
    /// A composition nothing has been done about yet.
    #[must_use]
    pub const fn pending(pin: DefinitionPin) -> Self {
        Self {
            instance_id: None,
            pin,
            status: LinkStatus::Pending,
            round: LoopRound::ZERO,
            skipped_because: None,
        }
    }

    /// The same link, now bound to a real instance.
    #[must_use]
    pub fn started(&self, instance_id: CeremonyId, round: LoopRound) -> Self {
        Self {
            instance_id: Some(instance_id),
            pin: self.pin.clone(),
            status: LinkStatus::Started,
            round,
            skipped_because: None,
        }
    }

    /// The same link, settled in an observed state.
    #[must_use]
    pub fn settled(&self, status: LinkStatus) -> Self {
        Self {
            instance_id: self.instance_id.clone(),
            pin: self.pin.clone(),
            status,
            round: self.round,
            skipped_because: self.skipped_because.clone(),
        }
    }

    /// The same link, skipped because somebody it needed was missing.
    #[must_use]
    pub fn skipped(&self, reason: UnavailabilityReason) -> Self {
        Self {
            instance_id: None,
            pin: self.pin.clone(),
            status: LinkStatus::Skipped,
            round: self.round,
            skipped_because: Some(reason),
        }
    }

    #[must_use]
    pub const fn instance_id(&self) -> Option<&CeremonyId> {
        self.instance_id.as_ref()
    }

    #[must_use]
    pub const fn pin(&self) -> &DefinitionPin {
        &self.pin
    }

    #[must_use]
    pub const fn status(&self) -> LinkStatus {
        self.status
    }

    #[must_use]
    pub const fn round(&self) -> LoopRound {
        self.round
    }

    #[must_use]
    pub const fn skipped_because(&self) -> Option<&UnavailabilityReason> {
        self.skipped_because.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use crate::value_objects::{CeremonyDefinitionDigest, CeremonyName, CeremonyVersion};

    use super::*;

    fn link() -> CeremonyExecutionLink {
        CeremonyExecutionLink::pending(DefinitionPin::new(
            CeremonyName::new("review").unwrap(),
            CeremonyVersion::new("1.0").unwrap(),
            CeremonyDefinitionDigest::from_bytes([7; 32]),
        ))
    }

    #[test]
    fn a_skipped_link_keeps_its_reason_and_forgets_no_instance() {
        let skipped = link().skipped(UnavailabilityReason::new("no reviewer available").unwrap());

        assert_eq!(skipped.status(), LinkStatus::Skipped);
        assert!(skipped.instance_id().is_none());
        assert!(skipped.skipped_because().is_some());
    }

    #[test]
    fn settling_a_started_link_keeps_the_instance_it_names() {
        let started = link().started(CeremonyId::new("c-1").unwrap(), LoopRound::ZERO.next());
        let completed = started.settled(LinkStatus::Completed);

        assert_eq!(completed.instance_id().unwrap().as_str(), "c-1");
        assert_eq!(completed.round().get(), 1);
    }
}
