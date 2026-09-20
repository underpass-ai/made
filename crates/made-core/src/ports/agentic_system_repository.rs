//! [`AgenticSystemRepositoryPort`] — the revision log of designs.

use async_trait::async_trait;

use crate::entities::AgenticSystem;
use crate::error::DomainError;
use crate::value_objects::{AgenticSystemId, AgenticSystemRevision};

use super::{AgenticSystemPage, AgenticSystemQuery, AgenticSystemSaveOutcome};

/// Persistence for agentic system designs, one row per revision.
///
/// Compare-and-swap is the whole contract. Two people editing one
/// design is the normal case, not the exceptional one, and a save that
/// simply overwrote would lose whichever edit landed first without
/// anybody finding out. `expected` is the revision the editor read;
/// `None` means "this design should not exist yet" and is the only way
/// to create one.
///
/// Not event sourced, deliberately (ADR-021): a design has a handful
/// of revisions produced by somebody editing it and no decision stream
/// of its own, and the one property that matters here — no lost update
/// — is exactly what compare-and-swap gives.
#[async_trait]
pub trait AgenticSystemRepositoryPort: Send + Sync {
    /// Store a revision, or report which revision is actually current.
    ///
    /// Reading the head and writing must be one indivisible step:
    /// checked beforehand, the answer is already stale by the time the
    /// write lands.
    async fn save(
        &self,
        system: AgenticSystem,
        expected: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemSaveOutcome, DomainError>;

    /// One design, at a named revision or at its head.
    async fn get(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
    ) -> Result<Option<AgenticSystem>, DomainError>;

    /// A bounded page of heads.
    async fn list(&self, query: &AgenticSystemQuery) -> Result<AgenticSystemPage, DomainError>;
}
