use super::protocol_work_candidate::ProtocolWorkCandidate;
use super::protocol_worker_claim_error::ProtocolWorkerClaimError;
use super::{
    ProtocolWorkerAdmissionObserver, ProtocolWorkerAdmissionPolicy, ProtocolWorkerAdmissionReason,
    ProtocolWorkerEligibility, ProtocolWorkerRootPolicyPort, ProtocolWorkerScheduleRequest,
    ProtocolWorkerScheduler, WorkerAuthorizationError, WorkerAuthorizationPort,
    WorkerAuthorizationTarget, WorkerCapacityPort, WorkerCapacityRequest,
};
use made_core::value_objects::{ExecutionConnectorId, ExecutionOperationId, StepLease};
use std::sync::{Arc, Mutex};

use made_core::error::DomainError;
use made_core::ports::{
    BudgetReservationPlannerPort, ClockPort, ProtocolInstanceIndexPort, ProtocolStepHandlerRequest,
};
use made_core::value_objects::{
    BudgetReservationRequest, IdempotencyKey, ProtocolId, ProtocolStep, StepId,
};

use super::{
    ClaimProtocolWorkInput, ExecuteProtocolOperationInput, ProtocolWorkClaimFailure,
    ProtocolWorkClaimsPage, ProtocolWorkerPolicy,
};
use crate::budgets::{BudgetedStepClaimInput, BudgetedStepClaimUseCase};
use crate::services::AuthorizationOperationScope;
use crate::services::{protocol_transcript_projection, SessionStream};
use crate::usecases::{
    EnforceProtocolDeadlinesInput, EnforceProtocolDeadlinesUseCase, ProtocolInstanceView,
    ResolveProtocolDefinitionUseCase, StartProtocolStepInput, StartProtocolStepOutput,
    StartProtocolStepUseCase,
};

/// Discovers protocols in keyset order and claims at most one external step per protocol.
pub struct ClaimProtocolWorkUseCase {
    index: Arc<dyn ProtocolInstanceIndexPort>,
    stream: Arc<SessionStream>,
    definitions: Arc<ResolveProtocolDefinitionUseCase>,
    deadlines: Arc<EnforceProtocolDeadlinesUseCase>,
    start_step: Arc<StartProtocolStepUseCase>,
    budgeted_step: Option<Arc<BudgetedStepClaimUseCase>>,
    budget_planner: Option<Arc<dyn BudgetReservationPlannerPort>>,
    clock: Arc<dyn ClockPort>,
    policy: ProtocolWorkerPolicy,
    scheduler: Mutex<ProtocolWorkerScheduler>,
    capacity: Option<(Arc<dyn WorkerCapacityPort>, ExecutionConnectorId)>,
    authorization: Option<Arc<dyn WorkerAuthorizationPort>>,
    admission: ProtocolWorkerAdmissionPolicy,
    root_policy: Option<Arc<dyn ProtocolWorkerRootPolicyPort>>,
}

impl std::fmt::Debug for ClaimProtocolWorkUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClaimProtocolWorkUseCase")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl ClaimProtocolWorkUseCase {
    #[must_use]
    pub fn new(
        index: Arc<dyn ProtocolInstanceIndexPort>,
        stream: Arc<SessionStream>,
        definitions: Arc<ResolveProtocolDefinitionUseCase>,
        deadlines: Arc<EnforceProtocolDeadlinesUseCase>,
        start_step: Arc<StartProtocolStepUseCase>,
        clock: Arc<dyn ClockPort>,
        policy: ProtocolWorkerPolicy,
    ) -> Self {
        let admission = ProtocolWorkerAdmissionPolicy::new(
            super::ProtocolWorkerSchedulerPolicy::new(
                policy.max_parallel(),
                super::ProtocolWorkerCapacity::new(1).expect("one is valid capacity"),
                super::ProtocolWorkerPolicyVersion::new(1).expect("one is valid version"),
            ),
            super::ProtocolWorkerPriority::DEFAULT,
            super::ProtocolWorkerWeight::new(1).expect("one is valid weight"),
            super::ProtocolWorkerCost::new(1).expect("one is valid cost"),
            super::ProtocolWorkerCapacity::new(1).expect("one is valid capacity"),
        );
        Self {
            index,
            stream,
            definitions,
            deadlines,
            start_step,
            budgeted_step: None,
            budget_planner: None,
            clock,
            policy,
            scheduler: Mutex::new(ProtocolWorkerScheduler::new(admission.scheduler())),
            capacity: None,
            authorization: None,
            admission,
            root_policy: None,
        }
    }

    #[must_use]
    pub fn with_budget_admission(
        mut self,
        budgeted_step: Arc<BudgetedStepClaimUseCase>,
        planner: Arc<dyn BudgetReservationPlannerPort>,
    ) -> Self {
        self.budgeted_step = Some(budgeted_step);
        self.budget_planner = Some(planner);
        self
    }

    #[must_use]
    pub fn with_shared_capacity(
        mut self,
        capacity: Arc<dyn WorkerCapacityPort>,
        connector: ExecutionConnectorId,
    ) -> Self {
        self.capacity = Some((capacity, connector));
        self
    }

    #[must_use]
    pub fn with_authorization(mut self, authorization: Arc<dyn WorkerAuthorizationPort>) -> Self {
        self.authorization = Some(authorization);
        self
    }

    #[must_use]
    pub fn with_admission_policy(mut self, admission: ProtocolWorkerAdmissionPolicy) -> Self {
        self.scheduler = Mutex::new(ProtocolWorkerScheduler::new(admission.scheduler()));
        self.admission = admission;
        self
    }

    #[must_use]
    pub fn with_admission_observer(
        mut self,
        observer: Arc<dyn ProtocolWorkerAdmissionObserver>,
    ) -> Self {
        self.scheduler = Mutex::new(ProtocolWorkerScheduler::with_observer(
            self.admission.scheduler(),
            observer,
        ));
        self
    }

    #[must_use]
    pub fn with_root_policy(mut self, policy: Arc<dyn ProtocolWorkerRootPolicyPort>) -> Self {
        self.root_policy = Some(policy);
        self
    }

    pub async fn execute(
        &self,
        input: ClaimProtocolWorkInput,
    ) -> Result<ProtocolWorkClaimsPage, DomainError> {
        let page = self
            .index
            .ids_after(input.after(), None, input.limit())
            .await?;
        let next_cursor = page
            .has_more()
            .then(|| page.ids().last().cloned())
            .flatten();
        let mut claims = Vec::new();
        let (candidates, mut failures) = Box::pin(self.discover_candidates(page.ids())).await;
        let requests = candidates.iter().map(|candidate| {
            let policy = self.root_policy.as_ref().map_or_else(
                || self.admission.root_policy(),
                |policy| policy.policy_for(&candidate.root),
            );
            ProtocolWorkerScheduleRequest::new(
                candidate.root.clone(),
                candidate.operation.clone(),
                policy.priority(),
                policy.weight(),
                policy.cost(),
                policy.requested_capacity(),
                ProtocolWorkerEligibility::Ready,
            )
        });
        let schedule = self
            .scheduler
            .lock()
            .map_err(|_| DomainError::InvariantViolated {
                reason: "worker scheduler state is unavailable",
            })?
            .refresh(requests);
        for decision in schedule.admitted() {
            let candidate = candidates
                .iter()
                .find(|candidate| &candidate.operation == decision.request().operation_id())
                .expect("selected candidate came from discovery");
            match self
                .admit_candidate(candidate, decision.request(), &input)
                .await
            {
                Ok(Some(claim)) => claims.push(claim),
                Ok(None) => {}
                Err(error) => failures.push(ProtocolWorkClaimFailure::new(
                    candidate.protocol.clone(),
                    error.into_domain(),
                )),
            }
        }
        Ok(ProtocolWorkClaimsPage::new(claims, failures, next_cursor))
    }

    async fn admit_candidate(
        &self,
        candidate: &ProtocolWorkCandidate,
        request: &ProtocolWorkerScheduleRequest,
        input: &ClaimProtocolWorkInput,
    ) -> Result<Option<ExecuteProtocolOperationInput>, ProtocolWorkerClaimError> {
        if !self.reserve_capacity(candidate, input).await? {
            self.observe(request.clone(), ProtocolWorkerAdmissionReason::Capacity)?;
            return Ok(None);
        }
        let result = Box::pin(self.claim_authorized(candidate, input)).await;
        match result {
            Ok(Some(claim)) => {
                if let Some((capacity, _)) = &self.capacity {
                    capacity
                        .bind(
                            &candidate.operation,
                            input.lease_owner_id(),
                            &claim.claim_fence,
                        )
                        .await?;
                }
                Ok(Some(claim))
            }
            result => {
                if let Some((capacity, _)) = &self.capacity {
                    capacity
                        .release(&candidate.operation, input.lease_owner_id())
                        .await?;
                }
                if let Err(error) = &result {
                    let reason = match error {
                        ProtocolWorkerClaimError::Permission(_) => {
                            Some(ProtocolWorkerAdmissionReason::Permission)
                        }
                        ProtocolWorkerClaimError::Budget(_) => {
                            Some(ProtocolWorkerAdmissionReason::Budget)
                        }
                        ProtocolWorkerClaimError::Failure(_) => None,
                    };
                    if let Some(reason) = reason {
                        self.observe(request.clone(), reason)?;
                    }
                }
                result
            }
        }
    }

    async fn reserve_capacity(
        &self,
        candidate: &ProtocolWorkCandidate,
        input: &ClaimProtocolWorkInput,
    ) -> Result<bool, DomainError> {
        let Some((capacity, connector)) = &self.capacity else {
            return Ok(true);
        };
        let now = self.clock.now();
        let request = WorkerCapacityRequest {
            operation_id: candidate.operation.clone(),
            protocol_id: candidate.protocol.clone(),
            step_id: candidate.step.clone(),
            root_id: candidate.root.clone(),
            connector_id: connector.clone(),
            provider_id: candidate.provider.clone(),
            owner_id: input.lease_owner_id().clone(),
            pending_until: StepLease::acquire(
                input.lease_owner_id().clone(),
                IdempotencyKey::new("capacity-pending")?,
                now,
                input.lease_ttl(),
            )?
            .expires_at(),
        };
        capacity.reserve(&request, now).await
    }

    fn observe(
        &self,
        request: ProtocolWorkerScheduleRequest,
        reason: ProtocolWorkerAdmissionReason,
    ) -> Result<(), DomainError> {
        self.scheduler
            .lock()
            .map_err(|_| DomainError::InvariantViolated {
                reason: "worker scheduler state is unavailable",
            })?
            .observe(request, reason);
        Ok(())
    }

    async fn discover_candidates(
        &self,
        protocol_ids: &[ProtocolId],
    ) -> (Vec<ProtocolWorkCandidate>, Vec<ProtocolWorkClaimFailure>) {
        let mut candidates = Vec::new();
        let mut failures = Vec::new();
        for protocol_id in protocol_ids {
            let result = if let Some(authorization) = &self.authorization {
                let target = WorkerAuthorizationTarget::EnforceDeadline {
                    protocol: protocol_id.clone(),
                };
                let operation = authorization.authorize(&target).await;
                match operation {
                    Ok(operation) => {
                        Box::pin(AuthorizationOperationScope::run(
                            operation,
                            self.discover_one(protocol_id),
                        ))
                        .await
                    }
                    Err(error) => Err(error.into_domain()),
                }
            } else {
                self.discover_one(protocol_id).await
            };
            match result {
                Ok(Some(candidate)) => candidates.push(candidate),
                Ok(None) => {}
                Err(error) => {
                    failures.push(ProtocolWorkClaimFailure::new(protocol_id.clone(), error));
                }
            }
        }
        (candidates, failures)
    }

    async fn discover_one(
        &self,
        protocol_id: &ProtocolId,
    ) -> Result<Option<ProtocolWorkCandidate>, DomainError> {
        let instance = self
            .deadlines
            .execute(EnforceProtocolDeadlinesInput::new(protocol_id.clone()))
            .await?;
        let definition = self.definitions.execute(&instance).await?;
        let view = ProtocolInstanceView::project_at(
            &instance,
            &definition,
            self.clock.now(),
            self.policy.max_parallel(),
        )?;
        let Some(step_id) = view
            .claimable_step_ids()
            .iter()
            .copied()
            .find(|id| {
                definition
                    .step(id)
                    .is_some_and(|step| step.spawn().is_none() && self.connector_accepts(step))
            })
            .cloned()
        else {
            return Ok(None);
        };
        let record = instance
            .step_record(&step_id)
            .ok_or(DomainError::NotFound {
                what: "protocol_step",
            })?;
        let step = definition.step(&step_id).ok_or(DomainError::NotFound {
            what: "protocol_step",
        })?;
        let provider = step
            .handler_config()
            .attributes()
            .get("provider")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(step.handler_kind().as_str());
        Ok(Some(ProtocolWorkCandidate {
            protocol: protocol_id.clone(),
            root: instance
                .lineage()
                .map_or_else(|| protocol_id.clone(), |lineage| lineage.root_id().clone()),
            operation: ExecutionOperationId::for_step(
                protocol_id,
                &step_id,
                record.state_visit(),
                record.state_iteration(),
                record.iteration(),
            ),
            step: step_id,
            provider: ExecutionConnectorId::new(provider)?,
        }))
    }

    async fn claim_authorized(
        &self,
        candidate: &ProtocolWorkCandidate,
        input: &ClaimProtocolWorkInput,
    ) -> Result<Option<ExecuteProtocolOperationInput>, ProtocolWorkerClaimError> {
        let Some(authorization) = &self.authorization else {
            return self
                .claim_one(&candidate.protocol, input, Some(&candidate.step))
                .await;
        };
        let target = WorkerAuthorizationTarget::Claim {
            protocol: candidate.protocol.clone(),
            step: candidate.step.clone(),
            operation: candidate.operation.clone(),
            owner: input.lease_owner_id().clone(),
            lease_ttl: input.lease_ttl(),
        };
        let operation = authorization
            .authorize(&target)
            .await
            .map_err(|error| match error {
                WorkerAuthorizationError::Denied(error) => {
                    ProtocolWorkerClaimError::Permission(error)
                }
                WorkerAuthorizationError::Failure(error) => {
                    ProtocolWorkerClaimError::Failure(error)
                }
            })?;
        Box::pin(AuthorizationOperationScope::run(
            operation,
            self.claim_one(&candidate.protocol, input, Some(&candidate.step)),
        ))
        .await
    }

    fn connector_accepts(&self, step: &ProtocolStep) -> bool {
        let Some((_, connector)) = &self.capacity else {
            return true;
        };
        step.handler_config()
            .attributes()
            .get("connector")
            .and_then(serde_json::Value::as_str)
            .map_or_else(
                || step.handler_kind().as_str() == connector.as_str(),
                |configured| configured == connector.as_str(),
            )
    }

    async fn claim_one(
        &self,
        protocol_id: &ProtocolId,
        input: &ClaimProtocolWorkInput,
        selected_step: Option<&StepId>,
    ) -> Result<Option<ExecuteProtocolOperationInput>, ProtocolWorkerClaimError> {
        let instance = self
            .deadlines
            .execute(EnforceProtocolDeadlinesInput::new(protocol_id.clone()))
            .await?;
        let definition = self.definitions.execute(&instance).await?;
        let view = ProtocolInstanceView::project_at(
            &instance,
            &definition,
            self.clock.now(),
            self.policy.max_parallel(),
        )?;
        let step_id = view
            .claimable_step_ids()
            .iter()
            .copied()
            .find(|step_id| {
                selected_step.is_none_or(|selected| selected == *step_id)
                    && definition
                        .step(step_id)
                        .is_some_and(|step| step.spawn().is_none())
            })
            .cloned();
        let Some(step_id) = step_id else {
            return Ok(None);
        };
        let transcript =
            protocol_transcript_projection::transcript(&self.stream.records(protocol_id).await?);
        let role_id = definition.role_id_for_step(&step_id)?;
        let idempotency_key = Self::idempotency_key(&instance, &step_id)?;
        let step = definition.step(&step_id).ok_or(DomainError::NotFound {
            what: "protocol_step",
        })?;
        let claim_input = StartProtocolStepInput::new(
            protocol_id.clone(),
            role_id,
            input.actor_kind(),
            step_id.clone(),
            input.lease_owner_id().clone(),
            idempotency_key,
            input.lease_ttl(),
        )
        .with_automatic_role_resolution();
        let claim = self.claim_step(&instance, step, claim_input).await?;
        let accepted = claim
            .instance()
            .step_record(&step_id)
            .ok_or(DomainError::NotFound {
                what: "protocol_step",
            })?;
        let sealed_role = accepted
            .claimed_role()
            .cloned()
            .unwrap_or(definition.role_id_for_step(&step_id)?);
        let request = ProtocolStepHandlerRequest::new(
            protocol_id.clone(),
            claim.instance().definition_name().clone(),
            claim.instance().definition_version().clone(),
            claim.instance().current_state().clone(),
            step_id,
            step.handler_kind().clone(),
            step.handler_config().clone(),
            claim.instance().context().clone(),
            claim.attempt(),
        )
        .with_transcript(transcript)
        .with_role(sealed_role.clone())
        .with_bound_specialty(claim.instance().bound_specialty(&sealed_role).cloned());
        Ok(Some(ExecuteProtocolOperationInput {
            handler_request: request,
            state_visit: accepted.state_visit(),
            state_iteration: accepted.state_iteration(),
            step_iteration: accepted.iteration(),
            claim_fence: claim.claim_fence().clone(),
            actor_kind: input.actor_kind(),
        }))
    }

    async fn claim_step(
        &self,
        instance: &made_core::entities::ProtocolInstance,
        step: &ProtocolStep,
        claim_input: StartProtocolStepInput,
    ) -> Result<StartProtocolStepOutput, ProtocolWorkerClaimError> {
        if instance.budget_account_id().is_none() {
            return self
                .start_step
                .execute(claim_input)
                .await
                .map_err(ProtocolWorkerClaimError::Failure);
        }
        let planner = self
            .budget_planner
            .as_ref()
            .ok_or(DomainError::InvariantViolated {
                reason: "budgeted worker claim requires a reservation planner",
            })?;
        let estimate = planner
            .estimate(&BudgetReservationRequest::new(
                instance.id().clone(),
                claim_input.step_id.clone(),
                step.handler_kind().clone(),
                step.handler_config().clone(),
                instance.context().clone(),
            ))
            .await
            .map_err(ProtocolWorkerClaimError::Failure)?;
        self.budgeted_step
            .as_ref()
            .ok_or(DomainError::InvariantViolated {
                reason: "budgeted worker claim requires a budget ledger",
            })?
            .execute(BudgetedStepClaimInput::new(claim_input, estimate))
            .await
            .map_err(|error| {
                let exhausted = matches!(error, made_core::BudgetError::Exhausted { .. });
                let domain = crate::budgets::budget_error_to_domain(error);
                if exhausted {
                    ProtocolWorkerClaimError::Budget(domain)
                } else {
                    ProtocolWorkerClaimError::Failure(domain)
                }
            })
            .map(|output| output.claim().clone())
    }

    fn idempotency_key(
        instance: &made_core::entities::ProtocolInstance,
        step_id: &StepId,
    ) -> Result<IdempotencyKey, DomainError> {
        let record = instance.step_record(step_id).ok_or(DomainError::NotFound {
            what: "protocol_step",
        })?;
        // One key per attempt the claim starts. The recorded attempt is not
        // enough: a pending record and the in-progress or failed record its
        // first claim leaves behind share it, so a claim that takes over an
        // expired lease or retries a failed attempt would replay the first
        // claim's key and be refused. The `attempt:` label keeps these keys
        // disjoint from the ones earlier versions recorded.
        IdempotencyKey::new(format!(
            "worker:{}:{}:{}:{}:{}:attempt:{}",
            instance.id(),
            step_id,
            record.state_visit().get(),
            record.state_iteration().get(),
            record.iteration().get(),
            record.next_start_attempt()?.get()
        ))
    }
}
