//! [`AgenticSystemExecutionStorePort`] — where runs of a design live.

use async_trait::async_trait;
use time::OffsetDateTime;

use crate::entities::AgenticSystemExecution;
use crate::error::DomainError;
use crate::value_objects::{AgenticSystemExecutionId, AgenticSystemId};

use super::{AgenticSystemExecutionCreation, AgenticSystemExecutionUpdate};

/// Persistence for runs, separate from the designs they run.
///
/// The design store protects an author's edits; this one protects a
/// run's progress, and they are different races between different
/// parties. Keeping them in one store would mean every advance of
/// every run contended with every edit of the design.
#[async_trait]
pub trait AgenticSystemExecutionStorePort: Send + Sync {
    /// Open a run, or hand back the one that identifier already names.
    async fn create(
        &self,
        execution: AgenticSystemExecution,
    ) -> Result<AgenticSystemExecutionCreation, DomainError>;

    async fn get(
        &self,
        id: &AgenticSystemExecutionId,
    ) -> Result<Option<AgenticSystemExecution>, DomainError>;

    /// Write a run back, against the moment the caller last saw it.
    ///
    /// The timestamp is the version: a run carries no revision of its
    /// own, and every write moves `updated_at`, so comparing it is
    /// what keeps two advances of the same run from each overwriting
    /// the other's links.
    async fn update(
        &self,
        execution: AgenticSystemExecution,
        expected_updated_at: OffsetDateTime,
    ) -> Result<AgenticSystemExecutionUpdate, DomainError>;

    /// Every run of one design, newest last.
    async fn list_by_system(
        &self,
        id: &AgenticSystemId,
    ) -> Result<Vec<AgenticSystemExecution>, DomainError>;
}
