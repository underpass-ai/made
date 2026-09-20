//! [`AgenticSystemExecution`] — one run of one sealed design.
//!
//! A separate entity with a separate store, on purpose (ADR-021). Held
//! on the design aggregate, every run would rewrite the document and
//! fight the compare-and-swap that protects an author's edits; and a
//! design whose bytes changed whenever somebody ran it could not be
//! pinned by the run at all.
//!
//! What it holds is the join between intent and reality: which design
//! it is running, which real instance each composition became, which
//! profiles were requested for each role, and how each logical
//! participant was materialized — including the ones that were not.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::DomainError;
use crate::value_objects::{
    AgenticSystemExecutionId, CeremonyExecutionLink, ExecutionState, IntegratorBindingId,
    LinkStatus, ParticipantId, ParticipantMaterialization, RequestedExecutionProfile,
    SystemCeremonyId, SystemPin, SystemRoleId,
};

/// One run of an agentic system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgenticSystemExecution {
    id: AgenticSystemExecutionId,
    system: SystemPin,
    ceremonies: BTreeMap<SystemCeremonyId, CeremonyExecutionLink>,
    resolved_profiles: BTreeMap<SystemRoleId, RequestedExecutionProfile>,
    participants: BTreeMap<ParticipantId, ParticipantMaterialization>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    integrator_binding: Option<IntegratorBindingId>,
    state: ExecutionState,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
}

impl AgenticSystemExecution {
    /// Open a run with nothing started yet.
    pub fn plan(
        id: AgenticSystemExecutionId,
        system: SystemPin,
        ceremonies: impl IntoIterator<Item = (SystemCeremonyId, CeremonyExecutionLink)>,
        resolved_profiles: impl IntoIterator<Item = (SystemRoleId, RequestedExecutionProfile)>,
        participants: impl IntoIterator<Item = (ParticipantId, ParticipantMaterialization)>,
        now: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        let ceremonies: BTreeMap<_, _> = ceremonies.into_iter().collect();
        if ceremonies.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "agentic_system_execution.ceremonies",
            });
        }
        let mut execution = Self {
            id,
            system,
            ceremonies,
            resolved_profiles: resolved_profiles.into_iter().collect(),
            participants: participants.into_iter().collect(),
            integrator_binding: None,
            state: ExecutionState::Planned,
            created_at: now,
            updated_at: now,
        };
        execution.state = execution.observed_state();
        Ok(execution)
    }

    #[must_use]
    pub const fn id(&self) -> &AgenticSystemExecutionId {
        &self.id
    }

    /// Which design, at which revision, with which bytes.
    #[must_use]
    pub const fn system(&self) -> &SystemPin {
        &self.system
    }

    #[must_use]
    pub const fn ceremonies(&self) -> &BTreeMap<SystemCeremonyId, CeremonyExecutionLink> {
        &self.ceremonies
    }

    /// What was asked of the host for each role, never what it did.
    #[must_use]
    pub const fn resolved_profiles(&self) -> &BTreeMap<SystemRoleId, RequestedExecutionProfile> {
        &self.resolved_profiles
    }

    #[must_use]
    pub const fn participants(&self) -> &BTreeMap<ParticipantId, ParticipantMaterialization> {
        &self.participants
    }

    #[must_use]
    pub const fn integrator_binding(&self) -> Option<&IntegratorBindingId> {
        self.integrator_binding.as_ref()
    }

    #[must_use]
    pub const fn state(&self) -> ExecutionState {
        self.state
    }

    #[must_use]
    pub const fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    #[must_use]
    pub const fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }

    #[must_use]
    pub fn link(&self, ceremony: &SystemCeremonyId) -> Option<&CeremonyExecutionLink> {
        self.ceremonies.get(ceremony)
    }

    /// The run with one composition's link replaced.
    ///
    /// Replacing rather than mutating, and re-deriving the state from
    /// the links each time, so the summary can never disagree with the
    /// detail it summarises.
    pub fn with_link(
        &self,
        ceremony: &SystemCeremonyId,
        link: CeremonyExecutionLink,
        now: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        if !self.ceremonies.contains_key(ceremony) {
            return Err(DomainError::NotFound {
                what: "agentic_system_execution.ceremony",
            });
        }
        let mut next = self.clone();
        next.ceremonies.insert(ceremony.clone(), link);
        next.updated_at = now;
        next.state = next.observed_state();
        Ok(next)
    }

    /// The run with the integrator's destination recorded.
    #[must_use]
    pub fn with_integrator_binding(
        &self,
        binding: IntegratorBindingId,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            integrator_binding: Some(binding),
            updated_at: now,
            ..self.clone()
        }
    }

    /// Which compositions could be started now: still pending, and
    /// with every predecessor completed.
    #[must_use]
    pub fn ready(
        &self,
        predecessors: &BTreeMap<SystemCeremonyId, Vec<SystemCeremonyId>>,
    ) -> Vec<&SystemCeremonyId> {
        self.ceremonies
            .iter()
            .filter(|(id, link)| {
                link.status() == LinkStatus::Pending
                    && predecessors
                        .get(*id)
                        .is_none_or(|waiting| waiting.iter().all(|other| self.is_completed(other)))
            })
            .map(|(id, _)| id)
            .collect()
    }

    fn is_completed(&self, ceremony: &SystemCeremonyId) -> bool {
        self.ceremonies
            .get(ceremony)
            .is_some_and(|link| link.status().releases_dependants())
    }

    fn observed_state(&self) -> ExecutionState {
        ExecutionState::of(self.ceremonies.values().map(CeremonyExecutionLink::status))
    }
}
