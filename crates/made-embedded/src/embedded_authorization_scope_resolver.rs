use async_trait::async_trait;
use made_core::ports::AuthorizationScopeResolverPort;
use made_core::value_objects::{AuthorizationScope, CeremonyId};
use made_core::DomainError;

use crate::EmbeddedMade;

#[async_trait]
impl AuthorizationScopeResolverPort for EmbeddedMade {
    async fn ceremony_scope(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<AuthorizationScope, DomainError> {
        match self.stream.load(ceremony_id).await {
            Ok(read) => Ok(read.instance.lineage().map_or_else(
                || AuthorizationScope::Ceremony {
                    ceremony_id: ceremony_id.clone(),
                },
                |lineage| AuthorizationScope::ResolvedCeremony {
                    root_id: lineage.root_id().clone(),
                    ceremony_id: ceremony_id.clone(),
                },
            )),
            Err(DomainError::NotFound { .. }) => Ok(AuthorizationScope::Ceremony {
                ceremony_id: ceremony_id.clone(),
            }),
            Err(error) => Err(error),
        }
    }

    async fn budget_scope(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<AuthorizationScope, DomainError> {
        let read = self.stream.load(ceremony_id).await?;
        let account_id = read
            .instance
            .budget_account_id()
            .ok_or(DomainError::NotFound {
                what: "ceremony_budget_account",
            })?;
        Ok(AuthorizationScope::Budget {
            account_id: account_id.clone(),
        })
    }

    async fn child_parent_scope(
        &self,
        child_id: &CeremonyId,
    ) -> Result<AuthorizationScope, DomainError> {
        let child = self.stream.load(child_id).await?;
        let lineage = child
            .instance
            .lineage()
            .ok_or(DomainError::InvariantViolated {
                reason: "child authorization scope requires sealed lineage",
            })?;
        self.ceremony_scope(lineage.parent_id()).await
    }
}
