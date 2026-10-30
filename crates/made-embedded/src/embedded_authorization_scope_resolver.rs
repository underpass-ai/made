use async_trait::async_trait;
use made_core::ports::AuthorizationScopeResolverPort;
use made_core::value_objects::{AuthorizationScope, ProtocolId};
use made_core::DomainError;

use crate::EmbeddedMade;

#[async_trait]
impl AuthorizationScopeResolverPort for EmbeddedMade {
    async fn protocol_scope(
        &self,
        protocol_id: &ProtocolId,
    ) -> Result<AuthorizationScope, DomainError> {
        match self.stream.load(protocol_id).await {
            Ok(read) => Ok(read.instance.lineage().map_or_else(
                || AuthorizationScope::Protocol {
                    protocol_id: protocol_id.clone(),
                },
                |lineage| AuthorizationScope::ResolvedProtocol {
                    root_id: lineage.root_id().clone(),
                    protocol_id: protocol_id.clone(),
                },
            )),
            Err(DomainError::NotFound { .. }) => Ok(AuthorizationScope::Protocol {
                protocol_id: protocol_id.clone(),
            }),
            Err(error) => Err(error),
        }
    }

    async fn budget_scope(
        &self,
        protocol_id: &ProtocolId,
    ) -> Result<AuthorizationScope, DomainError> {
        let read = self.stream.load(protocol_id).await?;
        let account_id = read
            .instance
            .budget_account_id()
            .ok_or(DomainError::NotFound {
                what: "protocol_budget_account",
            })?;
        Ok(AuthorizationScope::Budget {
            account_id: account_id.clone(),
        })
    }

    async fn child_parent_scope(
        &self,
        child_id: &ProtocolId,
    ) -> Result<AuthorizationScope, DomainError> {
        let child = self.stream.load(child_id).await?;
        let lineage = child
            .instance
            .lineage()
            .ok_or(DomainError::InvariantViolated {
                reason: "child authorization scope requires sealed lineage",
            })?;
        self.protocol_scope(lineage.parent_id()).await
    }
}
