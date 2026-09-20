//! [`AgenticSystem`] — the design of a system of agents, people and
//! ceremonies.
//!
//! A ceremony coordinates one procedure. This is the level above it: a
//! named system with business roles, logical participants, a
//! collaboration topology, several ceremonies composed together, and
//! the supervision the whole thing runs under.
//!
//! It references ceremonies and never owns them. Every composition
//! carries an immutable pin, and publishing resolves each pin against
//! what is actually published, so a design cannot quietly come to mean
//! something else because a version was republished underneath it
//! (ADR-021).
//!
//! It is not an execution engine either. A run is
//! [`AgenticSystemExecution`](super::AgenticSystemExecution), with its
//! own store, so a run never rewrites the design it is running.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::DomainError;
use crate::value_objects::{
    AgenticSystemDigest, AgenticSystemId, AgenticSystemLifecycle, AgenticSystemRevision,
    AttentionPolicy, CeremonyComposition, CollaborationLink, LogicalParticipant, ParticipantId,
    RequestedExecutionProfile, SupervisionPolicy, SystemCeremonyId, SystemPin, SystemPurpose,
    SystemRole, SystemRoleId,
};

mod agentic_system_content;
mod agentic_system_parts;

use agentic_system_content::AgenticSystemContent;

pub use agentic_system_parts::AgenticSystemParts;

/// One revision of one agentic system design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgenticSystem {
    id: AgenticSystemId,
    revision: AgenticSystemRevision,
    lifecycle: AgenticSystemLifecycle,
    purpose: SystemPurpose,
    integrator: SystemRoleId,
    roles: BTreeMap<SystemRoleId, SystemRole>,
    participants: BTreeMap<ParticipantId, LogicalParticipant>,
    topology: Vec<CollaborationLink>,
    profiles: BTreeMap<SystemRoleId, RequestedExecutionProfile>,
    ceremonies: BTreeMap<SystemCeremonyId, CeremonyComposition>,
    #[serde(default)]
    supervision: SupervisionPolicy,
    attention: AttentionPolicy,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
}

impl AgenticSystem {
    /// Start a design at its first revision.
    ///
    /// Deliberately permissive about the *content*: a draft is
    /// something an author is still working on, and refusing to write
    /// down a half-finished system would mean the only way to get
    /// advice about one is to have finished it. What is refused here
    /// is what no revision of the design could ever repair — an empty
    /// system, or one whose own indexes disagree with themselves.
    #[allow(clippy::too_many_arguments)]
    pub fn draft(
        id: AgenticSystemId,
        purpose: SystemPurpose,
        integrator: SystemRoleId,
        roles: impl IntoIterator<Item = SystemRole>,
        participants: impl IntoIterator<Item = LogicalParticipant>,
        topology: impl IntoIterator<Item = CollaborationLink>,
        profiles: impl IntoIterator<Item = (SystemRoleId, RequestedExecutionProfile)>,
        ceremonies: impl IntoIterator<Item = CeremonyComposition>,
        supervision: SupervisionPolicy,
        attention: AttentionPolicy,
        now: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        let roles = index(roles, SystemRole::id, "agentic_system.roles")?;
        let participants = index(
            participants,
            LogicalParticipant::id,
            "agentic_system.participants",
        )?;
        let ceremonies = index(
            ceremonies,
            CeremonyComposition::id,
            "agentic_system.ceremonies",
        )?;
        if roles.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "agentic_system.roles",
            });
        }
        if ceremonies.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "agentic_system.ceremonies",
            });
        }
        Ok(Self {
            id,
            revision: AgenticSystemRevision::INITIAL,
            lifecycle: AgenticSystemLifecycle::Draft,
            purpose,
            integrator,
            roles,
            participants,
            topology: topology.into_iter().collect(),
            profiles: profiles.into_iter().collect(),
            ceremonies,
            supervision,
            attention,
            created_at: now,
            updated_at: now,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &AgenticSystemId {
        &self.id
    }

    #[must_use]
    pub const fn revision(&self) -> AgenticSystemRevision {
        self.revision
    }

    #[must_use]
    pub const fn lifecycle(&self) -> AgenticSystemLifecycle {
        self.lifecycle
    }

    #[must_use]
    pub const fn purpose(&self) -> &SystemPurpose {
        &self.purpose
    }

    /// The business role that drives the system from outside it.
    #[must_use]
    pub const fn integrator(&self) -> &SystemRoleId {
        &self.integrator
    }

    #[must_use]
    pub const fn roles(&self) -> &BTreeMap<SystemRoleId, SystemRole> {
        &self.roles
    }

    #[must_use]
    pub const fn participants(&self) -> &BTreeMap<ParticipantId, LogicalParticipant> {
        &self.participants
    }

    #[must_use]
    pub fn topology(&self) -> &[CollaborationLink] {
        &self.topology
    }

    #[must_use]
    pub const fn profiles(&self) -> &BTreeMap<SystemRoleId, RequestedExecutionProfile> {
        &self.profiles
    }

    #[must_use]
    pub const fn ceremonies(&self) -> &BTreeMap<SystemCeremonyId, CeremonyComposition> {
        &self.ceremonies
    }

    #[must_use]
    pub const fn supervision(&self) -> &SupervisionPolicy {
        &self.supervision
    }

    /// What the integrator asked to be told about while it runs.
    #[must_use]
    pub const fn attention(&self) -> &AttentionPolicy {
        &self.attention
    }

    #[must_use]
    pub const fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    #[must_use]
    pub const fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }

    /// The content identity of this design.
    pub fn digest(&self) -> Result<AgenticSystemDigest, DomainError> {
        let canonical = serde_json::to_vec(&AgenticSystemContent::of(self)).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "agentic system cannot be rendered canonically",
            }
        })?;
        Ok(AgenticSystemDigest::of_canonical_form(&canonical))
    }

    /// This design, named the way a run refers to it.
    pub fn pin(&self) -> Result<SystemPin, DomainError> {
        Ok(SystemPin::new(
            self.id.clone(),
            self.revision,
            self.digest()?,
        ))
    }

    /// The same design saved as the next revision.
    ///
    /// Editing produces a new revision rather than mutating this one,
    /// because the store's compare-and-swap is what protects one
    /// author's edit from another's, and it can only compare revisions
    /// it was told about.
    #[must_use]
    pub fn edited(&self, now: OffsetDateTime) -> Self {
        Self {
            revision: self.revision.next(),
            lifecycle: AgenticSystemLifecycle::Draft,
            updated_at: now,
            ..self.clone()
        }
    }

    /// The same design, recorded at a revision the store assigned.
    #[must_use]
    pub fn at_revision(&self, revision: AgenticSystemRevision) -> Self {
        Self {
            revision,
            ..self.clone()
        }
    }

    /// Seal this revision.
    ///
    /// Refused from anything but a draft: publishing a published
    /// revision again would either be a no-op dressed as an event or a
    /// silent overwrite of something a run may already be pinned to.
    pub fn published(&self, now: OffsetDateTime) -> Result<Self, DomainError> {
        match self.lifecycle {
            AgenticSystemLifecycle::Draft => Ok(Self {
                lifecycle: AgenticSystemLifecycle::Published,
                updated_at: now,
                ..self.clone()
            }),
            AgenticSystemLifecycle::Published | AgenticSystemLifecycle::Deprecated => {
                Err(DomainError::InvalidTransition {
                    from: self.lifecycle.as_str(),
                    to: "published",
                })
            }
        }
    }

    /// Retire this revision from new runs without invalidating the
    /// ones already sealed against it.
    pub fn deprecated(&self, now: OffsetDateTime) -> Result<Self, DomainError> {
        match self.lifecycle {
            AgenticSystemLifecycle::Published => Ok(Self {
                lifecycle: AgenticSystemLifecycle::Deprecated,
                updated_at: now,
                ..self.clone()
            }),
            AgenticSystemLifecycle::Draft | AgenticSystemLifecycle::Deprecated => {
                Err(DomainError::InvalidTransition {
                    from: self.lifecycle.as_str(),
                    to: "deprecated",
                })
            }
        }
    }

    /// The compositions nothing else has to finish first.
    ///
    /// These are where a run begins. A design whose every composition
    /// waits for another has a cycle, and the analysis says so before
    /// a run has to discover it by starting nothing.
    #[must_use]
    pub fn root_ceremonies(&self) -> Vec<&CeremonyComposition> {
        self.ceremonies
            .values()
            .filter(|composition| composition.predecessors().is_empty())
            .collect()
    }
}

fn index<Item, Key>(
    items: impl IntoIterator<Item = Item>,
    key: impl Fn(&Item) -> &Key,
    field: &'static str,
) -> Result<BTreeMap<Key, Item>, DomainError>
where
    Key: Ord + Clone,
{
    let mut indexed = BTreeMap::new();
    for item in items {
        if indexed.insert(key(&item).clone(), item).is_some() {
            return Err(DomainError::AlreadyExists { what: field });
        }
    }
    Ok(indexed)
}
