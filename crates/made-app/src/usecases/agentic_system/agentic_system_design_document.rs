//! The document an author sends, and the only place the JSON of a
//! design is understood.
//!
//! Decoded straight into typed value objects rather than mapped by
//! each adapter in turn. Two adapters mapping the same JSON is two
//! places for them to disagree, and this design's whole point is that
//! the same document produces the same digest on either backend —
//! which is a property of there being one decoder, not of a test that
//! checks two.
//!
//! Unknown fields are refused. A design is written by an agent as
//! often as by a person, and a misspelled key silently ignored is a
//! supervision policy that quietly is not there.

use std::collections::BTreeMap;

use made_core::entities::AgenticSystem;
use made_core::error::DomainError;
use made_core::value_objects::{
    AgenticSystemId, AgenticSystemRevision, AttentionPolicy, CeremonyDefinitionDigest,
    CollaborationLink, LogicalParticipant, RequestedExecutionProfile, SupervisionPolicy,
    SystemCeremonyId, SystemPurpose, SystemRole, SystemRoleId,
};
use serde::Deserialize;
use time::OffsetDateTime;

use super::AgenticSystemCompositionDocument;

/// What an author wants their system to be.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgenticSystemDesignDocument {
    id: AgenticSystemId,
    /// The revision the author read before editing, absent when they
    /// believe this design does not exist yet.
    #[serde(default)]
    expected_revision: Option<AgenticSystemRevision>,
    purpose: SystemPurpose,
    integrator_role_id: SystemRoleId,
    roles: Vec<SystemRole>,
    #[serde(default)]
    participants: Vec<LogicalParticipant>,
    #[serde(default)]
    topology: Vec<CollaborationLink>,
    #[serde(default)]
    profiles: BTreeMap<SystemRoleId, RequestedExecutionProfile>,
    ceremonies: Vec<AgenticSystemCompositionDocument>,
    #[serde(default)]
    supervision: SupervisionPolicy,
    #[serde(default)]
    attention: Option<AttentionPolicy>,
}

impl AgenticSystemDesignDocument {
    #[must_use]
    pub const fn id(&self) -> &AgenticSystemId {
        &self.id
    }

    #[must_use]
    pub const fn expected_revision(&self) -> Option<AgenticSystemRevision> {
        self.expected_revision
    }

    #[must_use]
    pub fn compositions(&self) -> &[AgenticSystemCompositionDocument] {
        &self.ceremonies
    }

    /// The design this document describes.
    ///
    /// `resolved` supplies a digest for each composition whose author
    /// stated none. Nothing else is checked here: whether a pin names
    /// something publishable, whether the seating covers the seats and
    /// whether the whole thing can ever finish are validation's
    /// questions, and an author has to be able to write a system down
    /// before it is a correct one.
    pub fn into_draft(
        self,
        resolved: &BTreeMap<SystemCeremonyId, CeremonyDefinitionDigest>,
        now: OffsetDateTime,
    ) -> Result<AgenticSystem, DomainError> {
        let compositions = self
            .ceremonies
            .iter()
            .map(|composition| composition.to_composition(resolved.get(composition.id()).copied()))
            .collect::<Result<Vec<_>, _>>()?;
        AgenticSystem::draft(
            self.id,
            self.purpose,
            self.integrator_role_id,
            self.roles,
            self.participants,
            self.topology,
            self.profiles,
            compositions,
            self.supervision,
            self.attention.unwrap_or_default(),
            now,
        )
    }
}
