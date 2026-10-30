use serde::{Deserialize, Serialize};

use crate::value_objects::{FollowReplacement, HostDeliveryPolicy};

/// The terms an intervention is offered to a host under.
///
/// A named wrapper rather than the ledger's own policy in the item's
/// shape, because the two vocabularies answer to different people: the
/// ledger's terms are an operator's tuning, and this is what a
/// supervisor chose when they asked. Keeping the wrapper also means an
/// intervention's defaults can differ from a system-wide default
/// without either of them moving.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InterventionDeliveryPolicy(HostDeliveryPolicy);

impl InterventionDeliveryPolicy {
    #[must_use]
    pub const fn new(policy: HostDeliveryPolicy) -> Self {
        Self(policy)
    }

    /// The terms an intervention gets when the asker chose none.
    ///
    /// Pull-and-lease, because an agent that is working is already
    /// talking to the engine and asking it for its questions costs
    /// nothing; activation is for a host that has stopped asking.
    #[must_use]
    pub fn pull() -> Self {
        Self(HostDeliveryPolicy::pull())
    }

    /// The same terms, with the question following a replaced agent.
    #[must_use]
    pub fn following_replacement(self) -> Self {
        Self(self.0.following_replacement())
    }

    #[must_use]
    pub const fn host_policy(&self) -> &HostDeliveryPolicy {
        &self.0
    }

    #[must_use]
    pub fn into_host_policy(self) -> HostDeliveryPolicy {
        self.0
    }

    #[must_use]
    pub const fn follow_replacement(&self) -> FollowReplacement {
        self.0.follow_replacement()
    }
}

impl Default for InterventionDeliveryPolicy {
    fn default() -> Self {
        Self::pull()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_pulls_and_stays_with_the_process_that_was_asked() {
        let policy = InterventionDeliveryPolicy::default();
        assert_eq!(policy.follow_replacement(), FollowReplacement::Stay);
        assert_eq!(
            policy.following_replacement().follow_replacement(),
            FollowReplacement::Follow
        );
    }

    #[test]
    fn it_is_the_ledger_policy_on_the_wire() {
        let policy = InterventionDeliveryPolicy::default();
        assert_eq!(
            serde_json::to_value(&policy).unwrap(),
            serde_json::to_value(policy.host_policy()).unwrap()
        );
    }
}
