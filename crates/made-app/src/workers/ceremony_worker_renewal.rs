use super::{
    ExecuteCeremonyOperationInput, ExecutionRecoveryItem, RecoverableCeremonyWorkerOutcome,
    RecoverableCeremonyWorkerPort, RenewCeremonyStepLeaseUseCase, WorkerAuthorizationPort,
    WorkerAuthorizationTarget, WorkerCapacityPort,
};
use crate::services::AuthorizationOperationScope;
use made_core::error::DomainError;
use made_core::ports::ExecutionCancellation;
use made_core::value_objects::{DurationMs, ExecutionOperationId, LeaseOwnerId};
use std::sync::Arc;
use std::time::Duration;

/// Maintains authority while a connector runs and drains cancellation on renewal loss.
pub struct CeremonyWorkerRenewal {
    renewal: Arc<RenewCeremonyStepLeaseUseCase>,
    owner: LeaseOwnerId,
    ttl: DurationMs,
    heartbeat: Duration,
    capacity: Option<Arc<dyn WorkerCapacityPort>>,
    authorization: Option<Arc<dyn WorkerAuthorizationPort>>,
}

impl std::fmt::Debug for CeremonyWorkerRenewal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonyWorkerRenewal")
            .field("owner", &self.owner)
            .field("ttl", &self.ttl)
            .field("heartbeat", &self.heartbeat)
            .finish_non_exhaustive()
    }
}

impl CeremonyWorkerRenewal {
    pub fn new(
        renewal: Arc<RenewCeremonyStepLeaseUseCase>,
        owner: LeaseOwnerId,
        ttl: DurationMs,
        heartbeat: Duration,
    ) -> Result<Self, DomainError> {
        if heartbeat.is_zero() || heartbeat.as_millis() >= u128::from(ttl.get()) {
            return Err(DomainError::InvariantViolated {
                reason: "worker heartbeat must be positive and shorter than the lease",
            });
        }
        Ok(Self {
            renewal,
            owner,
            ttl,
            heartbeat,
            capacity: None,
            authorization: None,
        })
    }

    #[must_use]
    pub fn with_shared_capacity(mut self, capacity: Arc<dyn WorkerCapacityPort>) -> Self {
        self.capacity = Some(capacity);
        self
    }

    #[must_use]
    pub fn with_authorization(mut self, authorization: Arc<dyn WorkerAuthorizationPort>) -> Self {
        self.authorization = Some(authorization);
        self
    }

    pub async fn execute(
        &self,
        worker: &dyn RecoverableCeremonyWorkerPort,
        input: ExecuteCeremonyOperationInput,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        let ceremony = input.handler_request.instance_id().clone();
        let step = input.handler_request.step_id().clone();
        let fence = input.claim_fence.clone();
        let operation = ExecutionOperationId::for_step(
            &ceremony,
            &step,
            input.state_visit,
            input.state_iteration,
            input.step_iteration,
        );
        // Prove that this worker still owns live authority before an external
        // effect can start. The same operation also extends a short lease.
        if let Err(error) = self
            .renew_authority(&ceremony, &step, &fence, &operation)
            .await
        {
            if let Some(capacity) = &self.capacity {
                capacity.release(&operation, &self.owner).await?;
            }
            return Err(error);
        }
        let cancellation = ExecutionCancellation::new();
        let execution = worker.execute_claim_cancellable(input, cancellation.clone());
        tokio::pin!(execution);
        let result = loop {
            tokio::select! {
                biased;
                result = &mut execution => break result,
                () = tokio::time::sleep(self.heartbeat) => {
                    if let Err(error) = self.renew_authority(&ceremony, &step, &fence, &operation).await {
                        cancellation.cancel();
                        // A process adapter returns only after its execution group is reaped.
                        let _ = execution.await;
                        break Err(error);
                    }
                }
            }
        };
        if let Some(capacity) = &self.capacity {
            capacity.release(&operation, &self.owner).await?;
        }
        result
    }

    pub async fn recover(
        &self,
        worker: &dyn RecoverableCeremonyWorkerPort,
        item: ExecutionRecoveryItem,
    ) -> Result<RecoverableCeremonyWorkerOutcome, DomainError> {
        let ceremony = item.operation().ceremony_id().clone();
        let step = item.operation().step_id().clone();
        let operation = item.operation().operation_id().clone();
        let fence = item
            .current_claim_fence()
            .cloned()
            .ok_or(DomainError::InvariantViolated {
                reason: "execution recovery has no current accepted claim",
            })?;
        if let Err(error) = self
            .renew_authority(&ceremony, &step, &fence, &operation)
            .await
        {
            if let Some(capacity) = &self.capacity {
                capacity.release(&operation, &self.owner).await?;
            }
            return Err(error);
        }
        let cancellation = ExecutionCancellation::new();
        let recovery = worker.recover_cancellable(item, cancellation.clone());
        tokio::pin!(recovery);
        let result = loop {
            tokio::select! {
                biased;
                result = &mut recovery => break result,
                () = tokio::time::sleep(self.heartbeat) => {
                    if let Err(error) = self.renew_authority(&ceremony, &step, &fence, &operation).await {
                        cancellation.cancel();
                        let _ = recovery.await;
                        break Err(error);
                    }
                }
            }
        };
        if let Some(capacity) = &self.capacity {
            capacity.release(&operation, &self.owner).await?;
        }
        result
    }

    async fn renew_authority(
        &self,
        ceremony: &made_core::value_objects::CeremonyId,
        step: &made_core::value_objects::StepId,
        fence: &made_core::value_objects::StepClaimFence,
        operation: &ExecutionOperationId,
    ) -> Result<(), DomainError> {
        let renew = async {
            let _capacity_guard = if let Some(capacity) = &self.capacity {
                Some(capacity.lock_renewal(operation, &self.owner).await?)
            } else {
                None
            };
            self.renewal
                .execute(ceremony, step, fence, &self.owner, self.ttl)
                .await?;
            Ok(())
        };
        let Some(authorization) = &self.authorization else {
            return renew.await;
        };
        let target = WorkerAuthorizationTarget::Renew {
            ceremony: ceremony.clone(),
            step: step.clone(),
            operation: operation.clone(),
            fence: fence.clone(),
        };
        let authorized = authorization
            .authorize(&target)
            .await
            .map_err(super::WorkerAuthorizationError::into_domain)?;
        Box::pin(AuthorizationOperationScope::run(authorized, renew)).await
    }
}
