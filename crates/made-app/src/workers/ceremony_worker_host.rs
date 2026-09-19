use std::sync::Arc;
use std::time::Duration;

use made_core::error::DomainError;
use made_core::value_objects::ExecutionRecoveryCursor;
use tokio::time::sleep;

use super::{
    CeremonyWorkerDriver, CeremonyWorkerHostOutcome, CeremonyWorkerHostPolicy,
    CeremonyWorkerRunOutcome, ClaimCeremonyWorkInput, ClaimCeremonyWorkUseCase,
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
        let page = Box::pin(self.claims.execute(input)).await?;
        let (claims, failures, next_cursor) = page.into_parts();
        let batch = self.driver.execute_claims(claims).await?;
        Ok(CeremonyWorkerHostOutcome::new(batch, failures, next_cursor))
    }

    /// Keeps recovery and admission moving until the host receives a stop.
    ///
    /// Recovery is drained before each admission page. Cursors are kept only
    /// as loop state; the journal and its projections remain authoritative.
    /// The bounded sleep prevents an empty store from becoming a busy loop.
    pub async fn run_until_stopped(
        &self,
        input: ClaimCeremonyWorkInput,
        poll_interval: Duration,
    ) -> Result<CeremonyWorkerRunOutcome, DomainError> {
        let mut recovery_cursor: Option<ExecutionRecoveryCursor> = None;
        let mut claim_input = input;
        let mut outcome = CeremonyWorkerRunOutcome::default();

        while !self.driver.stop_requested() {
            let mut recovery_stopped = false;
            if self.driver.recovery_enabled() {
                let recovery = self.driver.recover_page(recovery_cursor.as_ref()).await?;
                outcome.record_recovery_page(&recovery);
                recovery_cursor = recovery.next_cursor().cloned();
                recovery_stopped = recovery.stopped();
            }

            if recovery_cursor.is_none() && !recovery_stopped {
                let claimed = Box::pin(self.run_claim_page(claim_input.clone())).await?;
                outcome.record_claim_page(&claimed);
                claim_input = claim_input.with_after(claimed.next_cursor().cloned());
            }

            if !self.driver.stop_requested() {
                sleep(poll_interval).await;
            }
        }

        outcome.finish(true);
        Ok(outcome)
    }

    /// Run bounded claim and recovery pages until the shared stop token is
    /// requested. Every accepted batch is drained by the driver before this
    /// method observes stop; failed pages retry with bounded exponential
    /// backoff. No process or background task is created here.
    pub async fn run_continuously(
        &self,
        input: ClaimCeremonyWorkInput,
        policy: CeremonyWorkerHostPolicy,
    ) -> Result<CeremonyWorkerRunOutcome, DomainError> {
        let mut outcome = CeremonyWorkerRunOutcome::default();
        let mut claim_after = input.after().cloned();
        let mut recovery_after = None;
        let mut retry_attempt = 0_u32;

        while !self.driver.stop_requested() {
            let mut made_progress = false;
            let mut failed_page = false;
            for _ in 0..policy.max_pages_per_turn() {
                if self.driver.stop_requested() {
                    break;
                }
                let Ok(page) =
                    Box::pin(self.run_claim_page(input.with_after(claim_after.clone()))).await
                else {
                    outcome.record_retry();
                    retry_attempt = retry_attempt.saturating_add(1);
                    failed_page = true;
                    tokio::time::sleep(policy.backoff(retry_attempt)).await;
                    break;
                };
                retry_attempt = 0;
                made_progress |= !page.batch().outcomes().is_empty()
                    || !page.batch().failures().is_empty()
                    || !page.claim_failures().is_empty();
                claim_after = page.next_cursor().cloned();
                outcome.record_claim_page(&page);

                if self.driver.recovery_enabled() && !self.driver.stop_requested() {
                    let Ok(batch) = self.driver.recover_page(recovery_after.as_ref()).await else {
                        outcome.record_retry();
                        retry_attempt = retry_attempt.saturating_add(1);
                        failed_page = true;
                        tokio::time::sleep(policy.backoff(retry_attempt)).await;
                        break;
                    };
                    made_progress |= !batch.outcomes().is_empty() || !batch.failures().is_empty();
                    recovery_after = batch.next_cursor().cloned();
                    outcome.record_recovery_page(&batch);
                }

                if claim_after.is_none() && recovery_after.is_none() {
                    break;
                }
            }
            if self.driver.stop_requested() {
                break;
            }
            if failed_page {
                continue;
            }
            if made_progress {
                retry_attempt = 0;
            } else {
                retry_attempt = retry_attempt.saturating_add(1);
                tokio::time::sleep(policy.backoff(retry_attempt)).await;
            }
        }

        outcome.finish(self.driver.stop_requested());
        Ok(outcome)
    }
}
