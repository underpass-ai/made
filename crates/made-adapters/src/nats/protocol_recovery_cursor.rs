use async_trait::async_trait;
use made_app::usecases::{RecoverProtocolChildrenRound, RecoverProtocolChildrenUseCase};
use made_core::error::DomainError;
use made_core::value_objects::ProtocolEventPageLimit;

#[async_trait]
pub(super) trait RecoveryCursor: Send + Sync {
    async fn recover(
        &self,
        limit: ProtocolEventPageLimit,
    ) -> Result<RecoverProtocolChildrenRound, DomainError>;
}

#[async_trait]
impl RecoveryCursor for RecoverProtocolChildrenUseCase {
    async fn recover(
        &self,
        limit: ProtocolEventPageLimit,
    ) -> Result<RecoverProtocolChildrenRound, DomainError> {
        self.execute(limit).await
    }
}
