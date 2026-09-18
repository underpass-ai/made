use std::sync::Arc;

use futures::future::join_all;
use made_core::error::DomainError;
use made_core::value_objects::ExecutionRecoveryCursor;

use super::{
    CeremonyWorkerBatchOutcome, CeremonyWorkerPolicy, CeremonyWorkerStopToken,
    ExecuteCeremonyOperationInput, InspectExecutionRecoveryUseCase, RecoverableCeremonyWorkerPort,
};

/// Bounded host loop that drains every admitted chunk before observing stop.
pub struct CeremonyWorkerDriver {
    inspect_recovery: Option<Arc<InspectExecutionRecoveryUseCase>>,
    worker: Arc<dyn RecoverableCeremonyWorkerPort>,
    policy: CeremonyWorkerPolicy,
    stop: CeremonyWorkerStopToken,
}

impl std::fmt::Debug for CeremonyWorkerDriver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonyWorkerDriver")
            .field("policy", &self.policy)
            .field("stopped", &self.stop.is_requested())
            .finish_non_exhaustive()
    }
}

impl CeremonyWorkerDriver {
    #[must_use]
    pub const fn new(
        inspect_recovery: Arc<InspectExecutionRecoveryUseCase>,
        worker: Arc<dyn RecoverableCeremonyWorkerPort>,
        policy: CeremonyWorkerPolicy,
        stop: CeremonyWorkerStopToken,
    ) -> Self {
        Self {
            inspect_recovery: Some(inspect_recovery),
            worker,
            policy,
            stop,
        }
    }

    #[must_use]
    pub const fn for_claims(
        worker: Arc<dyn RecoverableCeremonyWorkerPort>,
        policy: CeremonyWorkerPolicy,
        stop: CeremonyWorkerStopToken,
    ) -> Self {
        Self {
            inspect_recovery: None,
            worker,
            policy,
            stop,
        }
    }

    pub async fn execute_claims(
        &self,
        claims: Vec<ExecuteCeremonyOperationInput>,
    ) -> Result<CeremonyWorkerBatchOutcome, DomainError> {
        let mut outcomes = Vec::with_capacity(claims.len());
        for chunk in claims.chunks(usize::from(self.policy.max_parallel().get())) {
            if self.stop.is_requested() {
                break;
            }
            let drained = join_all(
                chunk
                    .iter()
                    .cloned()
                    .map(|claim| self.worker.execute_claim(claim)),
            )
            .await;
            outcomes.extend(drained.into_iter().collect::<Result<Vec<_>, _>>()?);
        }
        Ok(CeremonyWorkerBatchOutcome::new(
            outcomes,
            None,
            self.stop.is_requested(),
        ))
    }

    pub async fn recover_page(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
    ) -> Result<CeremonyWorkerBatchOutcome, DomainError> {
        if self.stop.is_requested() {
            return Ok(CeremonyWorkerBatchOutcome::new(Vec::new(), None, true));
        }
        let inspect_recovery =
            self.inspect_recovery
                .as_ref()
                .ok_or(DomainError::InvariantViolated {
                    reason: "ceremony worker driver has no recovery inspector",
                })?;
        let page = inspect_recovery
            .execute(after, self.policy.recovery_page_limit())
            .await?;
        let (items, next_cursor) = page.into_parts();
        let mut outcomes = Vec::with_capacity(items.len());
        for chunk in items.chunks(usize::from(self.policy.max_parallel().get())) {
            if self.stop.is_requested() {
                break;
            }
            let drained =
                join_all(chunk.iter().cloned().map(|item| self.worker.recover(item))).await;
            outcomes.extend(drained.into_iter().collect::<Result<Vec<_>, _>>()?);
        }
        Ok(CeremonyWorkerBatchOutcome::new(
            outcomes,
            next_cursor,
            self.stop.is_requested(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use made_core::ports::CeremonyStepHandlerRequest;
    use made_core::value_objects::{
        Attributes, AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion,
        ExecutionOperationId, ExecutionRecoveryPageLimit, MaxParallel, StateIteration, StateVisit,
        StepAttempt, StepClaimFence, StepHandlerConfig, StepHandlerKind, StepId, StepIteration,
    };

    use super::*;
    use crate::workers::ExecutionRecoveryItem;

    #[derive(Debug)]
    struct GatedWorker {
        active: AtomicUsize,
        max_active: AtomicUsize,
        calls: AtomicUsize,
        stop: CeremonyWorkerStopToken,
    }

    #[async_trait]
    impl RecoverableCeremonyWorkerPort for GatedWorker {
        async fn execute_claim(
            &self,
            input: ExecuteCeremonyOperationInput,
        ) -> Result<crate::workers::RecoverableCeremonyWorkerOutcome, DomainError> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(active, Ordering::SeqCst);
            let calls = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if calls == 1 {
                self.stop.request();
            }
            tokio::task::yield_now().await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(
                crate::workers::RecoverableCeremonyWorkerOutcome::ReconciliationRequired(
                    ExecutionOperationId::for_step(
                        input.handler_request.instance_id(),
                        input.handler_request.step_id(),
                        input.state_visit,
                        input.state_iteration,
                        input.step_iteration,
                    ),
                ),
            )
        }

        async fn recover(
            &self,
            _item: ExecutionRecoveryItem,
        ) -> Result<crate::workers::RecoverableCeremonyWorkerOutcome, DomainError> {
            unreachable!("this test schedules accepted claims")
        }
    }

    fn claim(index: usize) -> ExecuteCeremonyOperationInput {
        ExecuteCeremonyOperationInput {
            handler_request: CeremonyStepHandlerRequest::new(
                CeremonyId::new(format!("ceremony-{index}")).unwrap(),
                CeremonyName::new("bounded_worker").unwrap(),
                CeremonyVersion::v1(),
                made_core::value_objects::StateId::new("OPEN").unwrap(),
                StepId::new("work").unwrap(),
                StepHandlerKind::new("fixture").unwrap(),
                StepHandlerConfig::new(Attributes::empty()),
                CeremonyContext::empty(),
                StepAttempt::FIRST,
            ),
            state_visit: StateVisit::FIRST,
            state_iteration: StateIteration::FIRST,
            step_iteration: StepIteration::FIRST,
            claim_fence: StepClaimFence::new(format!("{index:064x}")).unwrap(),
            actor_kind: AuditActorKind::Engine,
        }
    }

    #[tokio::test]
    async fn stop_drains_the_admitted_chunk_and_blocks_the_next_one() {
        let stop = CeremonyWorkerStopToken::new();
        let worker = Arc::new(GatedWorker {
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
            stop: stop.clone(),
        });
        let driver = CeremonyWorkerDriver::for_claims(
            worker.clone(),
            CeremonyWorkerPolicy::new(
                MaxParallel::new(2).unwrap(),
                ExecutionRecoveryPageLimit::new(5).unwrap(),
            ),
            stop,
        );

        let outcome = driver
            .execute_claims((1..=5).map(claim).collect())
            .await
            .unwrap();

        assert!(outcome.stopped());
        assert_eq!(outcome.outcomes().len(), 2);
        assert_eq!(worker.calls.load(Ordering::SeqCst), 2);
        assert_eq!(worker.max_active.load(Ordering::SeqCst), 2);
    }
}
