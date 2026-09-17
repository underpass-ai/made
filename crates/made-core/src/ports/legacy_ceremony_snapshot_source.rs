//! [`LegacyCeremonySnapshotSourcePort`] — what a pre-stream store can
//! still be asked.
//!
//! A store written before ceremonies were streams (ADR-012) holds
//! sessions the event-sourced engine cannot see: they have a snapshot
//! and a journal of payload-less receipts, and no stream. This is the
//! one door onto them, and it only opens outwards. It is read-only by
//! construction rather than by convention: there is no method that
//! writes, so the legacy tables cannot become a second write path
//! again — they are provenance now.

use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::LegacyCeremonySnapshot;

#[async_trait]
pub trait LegacyCeremonySnapshotSourcePort: Send + Sync {
    /// Every session the legacy tables hold that has no stream, sorted
    /// by id.
    ///
    /// Sorted because the import is one-shot evidence: two runs over
    /// the same store must name the same sessions in the same order,
    /// or a failed run could not be told apart from a different one.
    /// A session that already has a stream is not here — it was
    /// imported already, or it was never pre-stream.
    async fn instances_without_a_stream(&self) -> Result<Vec<LegacyCeremonySnapshot>, DomainError>;
}
