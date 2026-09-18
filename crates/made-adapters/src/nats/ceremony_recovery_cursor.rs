use async_trait::async_trait;
use made_app::usecases::{RecoverCeremonyChildrenRound, RecoverCeremonyChildrenUseCase};
use made_core::error::DomainError;
use made_core::value_objects::CeremonyEventPageLimit;

#[async_trait]
pub(super) trait RecoveryCursor: Send + Sync {
    async fn recover(
        &self,
        limit: CeremonyEventPageLimit,
    ) -> Result<RecoverCeremonyChildrenRound, DomainError>;
}

#[async_trait]
impl RecoveryCursor for RecoverCeremonyChildrenUseCase {
    async fn recover(
        &self,
        limit: CeremonyEventPageLimit,
    ) -> Result<RecoverCeremonyChildrenRound, DomainError> {
        self.execute(limit).await
    }
}
