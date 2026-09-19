use std::sync::Arc;

use made_core::error::DomainError;

use super::{
    CeremonyWorkerDriver, CeremonyWorkerHostOutcome, ClaimCeremonyWorkInput,
    ClaimCeremonyWorkUseCase,
};

/// Reference host pass: page ceremonies, claim through application, then drain work.
pub struct CeremonyWorkerHost {
    claims: Arc<ClaimCeremonyWorkUseCase>,
    driver: Arc<CeremonyWorkerDriver>,
}

impl std::fmt::Debug for CeremonyWorkerHost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonyWorkerHost")
            .finish_non_exhaustive()
    }
}

impl CeremonyWorkerHost {
    #[must_use]
    pub const fn new(
        claims: Arc<ClaimCeremonyWorkUseCase>,
        driver: Arc<CeremonyWorkerDriver>,
    ) -> Self {
        Self { claims, driver }
    }

    pub async fn run_claim_page(
        &self,
        input: ClaimCeremonyWorkInput,
    ) -> Result<CeremonyWorkerHostOutcome, DomainError> {
        let page = self.claims.execute(input).await?;
        let (claims, failures, next_cursor) = page.into_parts();
        let batch = self.driver.execute_claims(claims).await?;
        Ok(CeremonyWorkerHostOutcome::new(batch, failures, next_cursor))
    }
}
