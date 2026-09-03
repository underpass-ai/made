//! [`CeremonyCommit`] — everything one step of a ceremony changes.
//!
//! State, audit and publication are three claims about the same moment.
//! Saving them separately lets a process die between two of them and
//! leave a journal that disagrees with the state, or a message that
//! reports something that was never stored. They travel together so
//! they can land together.

use crate::entities::{AuditFact, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::{ExpectedRevision, OutboxMessage};

/// The unit that is committed, all of it or none of it.
#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyCommit {
    instance: CeremonyInstance,
    expected_revision: ExpectedRevision,
    facts: Vec<AuditFact>,
    messages: Vec<OutboxMessage>,
}

impl CeremonyCommit {
    /// Every fact must belong to the instance being committed.
    ///
    /// Rejected here rather than in the adapter: a commit that mixes
    /// ceremonies has no correct interpretation, and every
    /// implementation would otherwise have to discover that
    /// independently.
    pub fn new(
        instance: CeremonyInstance,
        expected_revision: ExpectedRevision,
        facts: impl IntoIterator<Item = AuditFact>,
        messages: impl IntoIterator<Item = OutboxMessage>,
    ) -> Result<Self, DomainError> {
        let facts = facts.into_iter().collect::<Vec<_>>();
        if facts.iter().any(|fact| &fact.ceremony_id != instance.id()) {
            return Err(DomainError::InvariantViolated {
                reason: "a commit cannot carry audit facts from another ceremony",
            });
        }
        Ok(Self {
            instance,
            expected_revision,
            facts,
            messages: messages.into_iter().collect(),
        })
    }

    #[must_use]
    pub fn instance(&self) -> &CeremonyInstance {
        &self.instance
    }

    #[must_use]
    pub fn expected_revision(&self) -> ExpectedRevision {
        self.expected_revision
    }

    #[must_use]
    pub fn facts(&self) -> &[AuditFact] {
        &self.facts
    }

    #[must_use]
    pub fn messages(&self) -> &[OutboxMessage] {
        &self.messages
    }

    /// Consume the commit into the parts an adapter writes.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        CeremonyInstance,
        ExpectedRevision,
        Vec<AuditFact>,
        Vec<OutboxMessage>,
    ) {
        (
            self.instance,
            self.expected_revision,
            self.facts,
            self.messages,
        )
    }
}
