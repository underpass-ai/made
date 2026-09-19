use async_trait::async_trait;

use crate::value_objects::{AuthorizationScope, CeremonyId};
use crate::DomainError;

/// Resolves authorization scopes from authoritative runtime state before a
/// protected facade operation reads or mutates that state.
#[async_trait]
pub trait AuthorizationScopeResolverPort: Send + Sync {
    async fn ceremony_scope(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<AuthorizationScope, DomainError>;

    async fn child_parent_scope(
        &self,
        child_id: &CeremonyId,
    ) -> Result<AuthorizationScope, DomainError>;

    async fn budget_scope(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<AuthorizationScope, DomainError>;
}
