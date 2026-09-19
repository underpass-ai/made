use std::sync::Arc;

use futures::future::join_all;
use made_core::error::DomainError;
use made_core::value_objects::ExecutionRecoveryCursor;

use super::{
    CeremonyDeadlineEnforcementPort, CeremonyWorkerBatchOutcome, CeremonyWorkerItemFailure,
    CeremonyWorkerPolicy, CeremonyWorkerStopToken, ExecuteCeremonyOperationInput,
    ExecutionRecoveryInspectorPort, ExecutionRecoveryItem, RecoverableCeremonyWorkerPort,
};

/// Bounded host loop that drains every admitted chunk before observing stop.
pub struct CeremonyWorkerDriver {
    inspect_recovery: Option<Arc<dyn ExecutionRecoveryInspectorPort>>,
    deadlines: Arc<dyn CeremonyDeadlineEnforcementPort>,
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
        inspect_recovery: Arc<dyn ExecutionRecoveryInspectorPort>,
        deadlines: Arc<dyn CeremonyDeadlineEnforcementPort>,
        worker: Arc<dyn RecoverableCeremonyWorkerPort>,
        policy: CeremonyWorkerPolicy,
        stop: CeremonyWorkerStopToken,
    ) -> Self {
        Self {
            inspect_recovery: Some(inspect_recovery),
            deadlines,
            worker,
            policy,
            stop,
        }
    }

    #[must_use]
    pub const fn for_claims(
        deadlines: Arc<dyn CeremonyDeadlineEnforcementPort>,
        worker: Arc<dyn RecoverableCeremonyWorkerPort>,
        policy: CeremonyWorkerPolicy,
        stop: CeremonyWorkerStopToken,
    ) -> Self {
        Self {
            inspect_recovery: None,
            deadlines,
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
        let mut failures = Vec::new();
        for chunk in claims.chunks(usize::from(self.policy.max_parallel().get())) {
            if self.stop.is_requested() {
                break;
            }
            let mut admitted = Vec::with_capacity(chunk.len());
            for claim in chunk {
                if self.stop.is_requested() {
                    break;
                }
                let current = self
                    .deadlines
                    .enforce_current_claim(
                        claim.handler_request.instance_id(),
                        claim.handler_request.step_id(),
                    )
                    .await?;
                if current.as_ref() == Some(&claim.claim_fence) {
                    admitted.push(claim.clone());
                }
            }
            let drained = join_all(admitted.into_iter().map(|claim| async {
                let ceremony_id = claim.handler_request.instance_id().clone();
                let step_id = claim.handler_request.step_id().clone();
                (ceremony_id, step_id, self.worker.execute_claim(claim).await)
            }))
            .await;
            for (ceremony_id, step_id, result) in drained {
                match result {
                    Ok(outcome) => outcomes.push(outcome),
                    Err(error) => {
                        failures.push(CeremonyWorkerItemFailure::new(ceremony_id, step_id, error));
                    }
                }
            }
        }
        Ok(CeremonyWorkerBatchOutcome::new(
            outcomes,
            failures,
            None,
            self.stop.is_requested(),
        ))
    }

    pub async fn recover_page(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
    ) -> Result<CeremonyWorkerBatchOutcome, DomainError> {
        if self.stop.is_requested() {
            return Ok(CeremonyWorkerBatchOutcome::new(
                Vec::new(),
                Vec::new(),
                None,
                true,
            ));
        }
        let inspect_recovery =
            self.inspect_recovery
                .as_ref()
                .ok_or(DomainError::InvariantViolated {
                    reason: "ceremony worker driver has no recovery inspector",
                })?;
        let page = inspect_recovery
            .inspect(after, self.policy.recovery_page_limit())
            .await?;
        let (items, next_cursor) = page.into_parts();
        let mut outcomes = Vec::with_capacity(items.len());
        let mut failures = Vec::new();
        for chunk in items.chunks(usize::from(self.policy.max_parallel().get())) {
            if self.stop.is_requested() {
                break;
            }
            let mut enforced = Vec::with_capacity(chunk.len());
            for item in chunk {
                if self.stop.is_requested() {
                    break;
                }
                let current_claim_fence = self
                    .deadlines
                    .enforce_current_claim(
                        item.operation().ceremony_id(),
                        item.operation().step_id(),
                    )
                    .await?;
                let (operation, intents, receipt, _) = item.clone().into_parts();
                enforced.push(ExecutionRecoveryItem::new(
                    operation,
                    intents,
                    receipt,
                    current_claim_fence,
                ));
            }
            let drained = join_all(enforced.into_iter().map(|item| async {
                let ceremony_id = item.operation().ceremony_id().clone();
                let step_id = item.operation().step_id().clone();
                (ceremony_id, step_id, self.worker.recover(item).await)
            }))
            .await;
            for (ceremony_id, step_id, result) in drained {
                match result {
                    Ok(outcome) => outcomes.push(outcome),
                    Err(error) => {
                        failures.push(CeremonyWorkerItemFailure::new(ceremony_id, step_id, error));
                    }
                }
            }
        }
        let stopped = self.stop.is_requested();
        Ok(CeremonyWorkerBatchOutcome::new(
            outcomes,
            failures,
            if stopped { None } else { next_cursor },
            stopped,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use async_trait::async_trait;
    use made_core::ports::CeremonyStepHandlerRequest;
    use made_core::value_objects::{
        Attributes, AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion,
        ExecutionConnectorId, ExecutionIntent, ExecutionOperation, ExecutionOperationId,
        ExecutionRecoveryCapability, ExecutionRecoveryPageLimit, ExecutionRequestBytes,
        MaxParallel, StateIteration, StateVisit, StepAttempt, StepClaimFence, StepHandlerConfig,
        StepHandlerKind, StepId, StepIteration,
    };
    use time::OffsetDateTime;

    use super::*;
    use crate::workers::{ExecutionRecoveryInspectorPort, ExecutionRecoveryItemsPage};

    #[derive(Debug)]
    struct DeadlineGate {
        current: Vec<(CeremonyId, StepClaimFence)>,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl CeremonyDeadlineEnforcementPort for DeadlineGate {
        async fn enforce_current_claim(
            &self,
            ceremony_id: &CeremonyId,
            _step_id: &StepId,
        ) -> Result<Option<StepClaimFence>, DomainError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .current
                .iter()
                .find(|(candidate, _)| candidate == ceremony_id)
                .map(|(_, fence)| fence.clone()))
        }
    }

    #[derive(Debug)]
    struct GatedWorker {
        active: AtomicUsize,
        max_active: AtomicUsize,
        calls: AtomicUsize,
        stop: CeremonyWorkerStopToken,
    }

    #[derive(Debug)]
    struct RecoveryInspector(Vec<ExecutionRecoveryItem>);

    #[async_trait]
    impl ExecutionRecoveryInspectorPort for RecoveryInspector {
        async fn inspect(
            &self,
            _after: Option<&ExecutionRecoveryCursor>,
            _limit: ExecutionRecoveryPageLimit,
        ) -> Result<ExecutionRecoveryItemsPage, DomainError> {
            Ok(ExecutionRecoveryItemsPage::new(
                self.0.clone(),
                Some(ExecutionRecoveryCursor::new("page-end").unwrap()),
            ))
        }
    }

    #[derive(Debug)]
    struct RecordingRecoveryWorker {
        recovered: Arc<Mutex<Vec<ExecutionOperationId>>>,
        stop: CeremonyWorkerStopToken,
        stop_on_first: bool,
    }

    #[derive(Debug)]
    struct PartlyFailingWorker;

    #[async_trait]
    impl RecoverableCeremonyWorkerPort for PartlyFailingWorker {
        async fn execute_claim(
            &self,
            input: ExecuteCeremonyOperationInput,
        ) -> Result<crate::workers::RecoverableCeremonyWorkerOutcome, DomainError> {
            if input.handler_request.instance_id().as_str() == "ceremony-1" {
                return Err(DomainError::InvariantViolated {
                    reason: "injected item failure",
                });
            }
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
            unreachable!("this test only executes fresh claims")
        }
    }

    #[async_trait]
    impl RecoverableCeremonyWorkerPort for RecordingRecoveryWorker {
        async fn execute_claim(
            &self,
            _input: ExecuteCeremonyOperationInput,
        ) -> Result<crate::workers::RecoverableCeremonyWorkerOutcome, DomainError> {
            unreachable!("this test only runs recovery")
        }

        async fn recover(
            &self,
            item: ExecutionRecoveryItem,
        ) -> Result<crate::workers::RecoverableCeremonyWorkerOutcome, DomainError> {
            let operation_id = item.operation().operation_id().clone();
            let mut recovered = self.recovered.lock().unwrap();
            recovered.push(operation_id.clone());
            if self.stop_on_first && recovered.len() == 1 {
                self.stop.request();
            }
            Ok(
                crate::workers::RecoverableCeremonyWorkerOutcome::ReconciliationRequired(
                    operation_id,
                ),
            )
        }
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

    fn recovery_item(index: usize) -> ExecutionRecoveryItem {
        let ceremony_id = CeremonyId::new(format!("recovery-{index}")).unwrap();
        let step_id = StepId::new("work").unwrap();
        let operation = ExecutionOperation::new(
            ceremony_id,
            step_id,
            StateVisit::FIRST,
            StateIteration::FIRST,
            StepIteration::FIRST,
            ExecutionRequestBytes::new(format!("request-{index}").into_bytes()).unwrap(),
        );
        let fence = StepClaimFence::new(format!("{index:064x}")).unwrap();
        let intent = ExecutionIntent::new(
            operation.clone(),
            fence.clone(),
            ExecutionConnectorId::new("test.recovery").unwrap(),
            ExecutionRecoveryCapability::ReconciliationRequired,
            made_core::value_objects::ArtifactSourceKind::NoOp,
            AuditActorKind::Engine,
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
        ExecutionRecoveryItem::new(operation, vec![intent], None, Some(fence))
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
        let claims = (1..=5).map(claim).collect::<Vec<_>>();
        let deadlines = Arc::new(DeadlineGate {
            current: claims
                .iter()
                .map(|claim| {
                    (
                        claim.handler_request.instance_id().clone(),
                        claim.claim_fence.clone(),
                    )
                })
                .collect(),
            calls: AtomicUsize::new(0),
        });
        let driver = CeremonyWorkerDriver::for_claims(
            deadlines.clone(),
            worker.clone(),
            CeremonyWorkerPolicy::new(
                MaxParallel::new(2).unwrap(),
                ExecutionRecoveryPageLimit::new(5).unwrap(),
            ),
            stop,
        );

        let outcome = driver.execute_claims(claims).await.unwrap();

        assert!(outcome.stopped());
        assert_eq!(outcome.outcomes().len(), 2);
        assert_eq!(worker.calls.load(Ordering::SeqCst), 2);
        assert_eq!(worker.max_active.load(Ordering::SeqCst), 2);
        assert_eq!(deadlines.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn an_expired_claim_is_not_admitted_after_deadline_enforcement() {
        let stop = CeremonyWorkerStopToken::new();
        let worker = Arc::new(GatedWorker {
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
            stop: stop.clone(),
        });
        let deadlines = Arc::new(DeadlineGate {
            current: Vec::new(),
            calls: AtomicUsize::new(0),
        });
        let driver = CeremonyWorkerDriver::for_claims(
            deadlines.clone(),
            worker.clone(),
            CeremonyWorkerPolicy::new(
                MaxParallel::new(1).unwrap(),
                ExecutionRecoveryPageLimit::new(5).unwrap(),
            ),
            stop,
        );

        let outcome = driver.execute_claims(vec![claim(1)]).await.unwrap();

        assert!(outcome.outcomes().is_empty());
        assert_eq!(worker.calls.load(Ordering::SeqCst), 0);
        assert_eq!(deadlines.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn stop_discards_the_page_cursor_so_resume_cannot_skip_an_undrained_item() {
        let items = (1..=3).map(recovery_item).collect::<Vec<_>>();
        let inspector = Arc::new(RecoveryInspector(items.clone()));
        let deadlines = Arc::new(DeadlineGate {
            current: items
                .iter()
                .map(|item| {
                    (
                        item.operation().ceremony_id().clone(),
                        item.current_claim_fence().unwrap().clone(),
                    )
                })
                .collect(),
            calls: AtomicUsize::new(0),
        });
        let recovered = Arc::new(Mutex::new(Vec::new()));
        let first_stop = CeremonyWorkerStopToken::new();
        let first = CeremonyWorkerDriver::new(
            inspector.clone(),
            deadlines.clone(),
            Arc::new(RecordingRecoveryWorker {
                recovered: recovered.clone(),
                stop: first_stop.clone(),
                stop_on_first: true,
            }),
            CeremonyWorkerPolicy::new(
                MaxParallel::new(1).unwrap(),
                ExecutionRecoveryPageLimit::new(3).unwrap(),
            ),
            first_stop,
        );
        let interrupted = first.recover_page(None).await.unwrap();
        assert!(interrupted.stopped());
        assert!(interrupted.next_cursor().is_none());

        let resumed = CeremonyWorkerDriver::new(
            inspector,
            deadlines,
            Arc::new(RecordingRecoveryWorker {
                recovered: recovered.clone(),
                stop: CeremonyWorkerStopToken::new(),
                stop_on_first: false,
            }),
            CeremonyWorkerPolicy::new(
                MaxParallel::new(1).unwrap(),
                ExecutionRecoveryPageLimit::new(3).unwrap(),
            ),
            CeremonyWorkerStopToken::new(),
        );
        let completed = resumed.recover_page(None).await.unwrap();

        assert!(!completed.stopped());
        assert!(completed.next_cursor().is_some());
        let mut operation_ids = recovered.lock().unwrap().clone();
        operation_ids.sort();
        operation_ids.dedup();
        assert_eq!(operation_ids.len(), 3);
    }

    #[tokio::test]
    async fn one_item_failure_keeps_the_successful_sibling_outcome() {
        let claims = vec![claim(1), claim(2)];
        let deadlines = Arc::new(DeadlineGate {
            current: claims
                .iter()
                .map(|claim| {
                    (
                        claim.handler_request.instance_id().clone(),
                        claim.claim_fence.clone(),
                    )
                })
                .collect(),
            calls: AtomicUsize::new(0),
        });
        let driver = CeremonyWorkerDriver::for_claims(
            deadlines,
            Arc::new(PartlyFailingWorker),
            CeremonyWorkerPolicy::new(
                MaxParallel::new(2).unwrap(),
                ExecutionRecoveryPageLimit::new(2).unwrap(),
            ),
            CeremonyWorkerStopToken::new(),
        );

        let outcome = driver.execute_claims(claims).await.unwrap();

        assert_eq!(outcome.outcomes().len(), 1);
        assert_eq!(outcome.failures().len(), 1);
        assert_eq!(outcome.failures()[0].ceremony_id().as_str(), "ceremony-1");
    }
}
