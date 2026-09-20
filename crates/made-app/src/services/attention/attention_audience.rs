//! Who a projection round is for, and what it may wake them about.

use std::collections::BTreeSet;

use made_core::error::DomainError;
use made_core::value_objects::{
    AgenticSystemExecutionId, AttentionPolicy, CeremonyEventConsumer, CeremonyId, IntegratorBinding,
};

/// One bound integrator, the ceremonies it drives, and what it asked to
/// be told about.
///
/// A binding names a scope rather than a list: an execution of a
/// composed system owns several ceremonies and opens more as it runs.
/// Resolving that scope into a set once per round, and carrying the
/// policy beside it, is what lets the projector walk the global feed
/// without asking a store what anything means halfway through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionAudience {
    binding: IntegratorBinding,
    policy: AttentionPolicy,
    covers: BTreeSet<CeremonyId>,
    system_execution_id: Option<AgenticSystemExecutionId>,
}

impl AttentionAudience {
    #[must_use]
    pub const fn new(
        binding: IntegratorBinding,
        policy: AttentionPolicy,
        covers: BTreeSet<CeremonyId>,
        system_execution_id: Option<AgenticSystemExecutionId>,
    ) -> Self {
        Self {
            binding,
            policy,
            covers,
            system_execution_id,
        }
    }

    #[must_use]
    pub const fn binding(&self) -> &IntegratorBinding {
        &self.binding
    }

    #[must_use]
    pub const fn policy(&self) -> &AttentionPolicy {
        &self.policy
    }

    #[must_use]
    pub const fn system_execution_id(&self) -> Option<&AgenticSystemExecutionId> {
        self.system_execution_id.as_ref()
    }

    /// Whether news from this ceremony is this integrator's business.
    ///
    /// The feed is global and one cursor walks all of it, so most
    /// records a round reads belong to somebody else. Being sure about
    /// that is the difference between a bounded consumer and a host
    /// woken for every ceremony in the deployment.
    #[must_use]
    pub fn covers(&self, ceremony_id: &CeremonyId) -> bool {
        self.covers.contains(ceremony_id)
    }

    /// The ceremonies in scope, in the order a store scans them.
    #[must_use]
    pub const fn ceremonies(&self) -> &BTreeSet<CeremonyId> {
        &self.covers
    }

    /// The durable cursor this audience reads the feed with.
    ///
    /// Named after the binding rather than the scope: replacing a host
    /// raises the fence and opens a new binding, and the replacement
    /// starts its own progress instead of inheriting a position whose
    /// deliveries went somewhere else.
    pub fn consumer(&self) -> Result<CeremonyEventConsumer, DomainError> {
        CeremonyEventConsumer::new(format!("attention:{}", self.binding.id()))
    }
}
