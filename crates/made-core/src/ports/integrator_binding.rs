//! Which integrator drives a ceremony or a system run, and since when.

use async_trait::async_trait;
use time::OffsetDateTime;

use crate::error::DomainError;
use crate::value_objects::{IntegratorBinding, IntegratorBindingId, IntegratorScope};

use super::{BindOutcome, BindReplacement};

/// Persistence contract for integrator bindings.
///
/// One live binding per scope. Replacing one raises its fence, so a
/// host that was swapped out is refused by comparison rather than
/// racing its replacement for the same ceremony.
#[async_trait]
pub trait IntegratorBindingPort: Send + Sync {
    /// Bind an integrator to a scope, displacing the incumbent or not.
    async fn bind(
        &self,
        binding: IntegratorBinding,
        replacement: BindReplacement,
    ) -> Result<BindOutcome, DomainError>;

    /// The binding in force for a scope, if any is.
    async fn current(
        &self,
        scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError>;

    /// End a binding. A binding that was already revoked stays as it was.
    async fn revoke(
        &self,
        id: &IntegratorBindingId,
        now: OffsetDateTime,
    ) -> Result<Option<IntegratorBinding>, DomainError>;

    /// Every binding, or every binding of one scope.
    async fn list(
        &self,
        scope: Option<&IntegratorScope>,
    ) -> Result<Vec<IntegratorBinding>, DomainError>;
}
