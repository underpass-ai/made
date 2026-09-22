use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{HostAgentIncarnation, RoleId};

use super::{
    HostDeliveryTarget, HostDestination, IntegratorBindingId, IntegratorFence, IntegratorScope,
    IntegratorScopeKey, LoopProgressMark,
};

/// One integrator's standing claim to drive a ceremony or a system run.
///
/// Fenced rather than merely replaced: a host that was swapped out
/// keeps its old fence, and every call it makes afterwards is refused
/// by comparison instead of racing the host that took its place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegratorBinding {
    id: IntegratorBindingId,
    scope: IntegratorScope,
    role_id: RoleId,
    destination: HostDestination,
    incarnation: HostAgentIncarnation,
    fence: IntegratorFence,
    #[serde(with = "time::serde::rfc3339")]
    bound_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    revoked_at: Option<OffsetDateTime>,
    /// Where this binding's loop had got to when it last asked.
    ///
    /// On the binding rather than beside it because it is the one
    /// thing per binding that only the loop writes and only the loop
    /// reads, and because a binding that is replaced should not hand
    /// its successor a count of rounds somebody else went.
    #[serde(default)]
    progress: LoopProgressMark,
}

impl IntegratorBinding {
    /// A first binding for a scope, at the opening fence.
    #[must_use]
    pub const fn new(
        id: IntegratorBindingId,
        scope: IntegratorScope,
        role_id: RoleId,
        destination: HostDestination,
        incarnation: HostAgentIncarnation,
        bound_at: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            scope,
            role_id,
            destination,
            incarnation,
            fence: IntegratorFence::FIRST,
            bound_at,
            revoked_at: None,
            progress: LoopProgressMark::new(None, 0, 0, 0),
        }
    }

    #[must_use]
    pub const fn id(&self) -> &IntegratorBindingId {
        &self.id
    }

    #[must_use]
    pub const fn scope(&self) -> &IntegratorScope {
        &self.scope
    }

    #[must_use]
    pub fn scope_key(&self) -> IntegratorScopeKey {
        self.scope.scope_key()
    }

    #[must_use]
    pub const fn role_id(&self) -> &RoleId {
        &self.role_id
    }

    #[must_use]
    pub const fn destination(&self) -> &HostDestination {
        &self.destination
    }

    #[must_use]
    pub const fn incarnation(&self) -> &HostAgentIncarnation {
        &self.incarnation
    }

    #[must_use]
    pub const fn fence(&self) -> IntegratorFence {
        self.fence
    }

    #[must_use]
    pub const fn bound_at(&self) -> OffsetDateTime {
        self.bound_at
    }

    #[must_use]
    pub const fn revoked_at(&self) -> Option<OffsetDateTime> {
        self.revoked_at
    }

    /// What this binding's loop saw the last time it asked.
    #[must_use]
    pub const fn progress(&self) -> LoopProgressMark {
        self.progress
    }

    /// The same binding, having asked once more.
    #[must_use]
    pub fn observing(&self, progress: LoopProgressMark) -> Self {
        Self {
            progress,
            ..self.clone()
        }
    }

    #[must_use]
    pub const fn is_live(&self) -> bool {
        self.revoked_at.is_none()
    }

    /// Whether a caller's claimed generation is the current one.
    #[must_use]
    pub fn admits(&self, incarnation: &HostAgentIncarnation, fence: IntegratorFence) -> bool {
        self.is_live() && &self.incarnation == incarnation && self.fence == fence
    }

    /// Where deliveries for this binding are addressed.
    #[must_use]
    pub fn delivery_target(&self) -> HostDeliveryTarget {
        HostDeliveryTarget::integrator_binding(self.id.clone())
    }

    /// The binding that takes this one's place, one fence higher.
    #[must_use]
    pub fn replacing(&self, replacement: &Self) -> Self {
        Self {
            fence: self.fence.next(),
            revoked_at: None,
            // The replacement starts its own loop. Inheriting a round
            // count it did not spend is how a fresh host arrives
            // already declared stuck.
            progress: LoopProgressMark::new(None, 0, 0, 0),
            ..replacement.clone()
        }
    }

    /// The same binding, no longer in force.
    #[must_use]
    pub fn revoked(&self, at: OffsetDateTime) -> Self {
        Self {
            revoked_at: Some(at),
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{CeremonyId, HostActivationMode, HostAddress, HostKind};

    fn binding(id: &str, incarnation: &str) -> IntegratorBinding {
        IntegratorBinding::new(
            IntegratorBindingId::new(id).unwrap(),
            IntegratorScope::ceremony(CeremonyId::new("c-1").unwrap()),
            RoleId::new("INTEGRATOR").unwrap(),
            HostDestination::new(
                HostKind::new("generic").unwrap(),
                HostAddress::new("session-1").unwrap(),
                HostActivationMode::None,
            ),
            HostAgentIncarnation::new(incarnation).unwrap(),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    #[test]
    fn a_replaced_binding_outranks_the_one_it_replaced() {
        let first = binding("b-1", "run-1");
        let second = first.replacing(&binding("b-2", "run-2"));
        assert_eq!(second.fence(), IntegratorFence::FIRST.next());
        assert!(second.admits(&HostAgentIncarnation::new("run-2").unwrap(), second.fence()));
        assert!(!second.admits(
            &HostAgentIncarnation::new("run-1").unwrap(),
            IntegratorFence::FIRST
        ));
    }

    #[test]
    fn a_revoked_binding_admits_nobody() {
        let revoked = binding("b-1", "run-1").revoked(OffsetDateTime::UNIX_EPOCH);
        assert!(!revoked.is_live());
        assert!(!revoked.admits(
            &HostAgentIncarnation::new("run-1").unwrap(),
            IntegratorFence::FIRST
        ));
    }
}
