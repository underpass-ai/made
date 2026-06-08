//! [`CeremonyContextStorePort`] — pluggable storage for a ceremony's
//! accumulating shared context (its transcript of interventions).
//!
//! Choreographer is agnostic about *where* ceremony context lives. The
//! default deployment keeps it in memory, but a downstream deployment can
//! back it with a durable store or the Underpass knowledge plane by
//! implementing this port — the domain never depends on the storage
//! mechanism. The engine only ever appends a contribution and reads back
//! the accumulated transcript.

use async_trait::async_trait;

use crate::error::DomainError;
use crate::value_objects::{CeremonyId, CeremonyStepContribution, CeremonyTranscript};

/// Stores and retrieves the transcript a ceremony accumulates as its
/// steps execute.
#[async_trait]
pub trait CeremonyContextStorePort: Send + Sync {
    /// Append a step's `contribution` to the transcript of `instance_id`.
    async fn append(
        &self,
        instance_id: &CeremonyId,
        contribution: CeremonyStepContribution,
    ) -> Result<(), DomainError>;

    /// The transcript accumulated for `instance_id` so far; an empty
    /// transcript when the ceremony has produced nothing yet.
    async fn transcript(&self, instance_id: &CeremonyId)
        -> Result<CeremonyTranscript, DomainError>;
}
