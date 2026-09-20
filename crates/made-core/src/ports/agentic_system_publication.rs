//! [`AgenticSystemPublicationPort`] — immutable storage for sealed
//! designs.
//!
//! Separate from the repository, whose `save` advances a head. A draft
//! being edited and a revision a run is pinned to are not the same
//! thing, and a store that could not tell them apart could not promise
//! the second is still what it was.

use async_trait::async_trait;

use crate::entities::{AgenticSystemPublicationOutcome, PublishedAgenticSystem};
use crate::error::DomainError;
use crate::value_objects::{AgenticSystemId, AgenticSystemRevision};

#[async_trait]
pub trait AgenticSystemPublicationPort: Send + Sync {
    /// Seal a revision, or report why it is unavailable.
    ///
    /// Idempotent by content: offering byte-identical content under a
    /// revision that already holds it answers `AlreadyPublished`,
    /// because a retried call is not a second publication. Offering
    /// different content under it is refused with both digests.
    async fn publish(
        &self,
        published: PublishedAgenticSystem,
    ) -> Result<AgenticSystemPublicationOutcome, DomainError>;

    async fn published(
        &self,
        id: &AgenticSystemId,
        revision: AgenticSystemRevision,
    ) -> Result<Option<PublishedAgenticSystem>, DomainError>;

    /// Every sealed design. Bounded by design rather than by a limit,
    /// like the ceremony catalogue it sits beside.
    async fn catalogue(&self) -> Result<Vec<PublishedAgenticSystem>, DomainError>;
}
