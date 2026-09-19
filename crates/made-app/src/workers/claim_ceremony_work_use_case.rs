use super::ceremony_work_candidate::CeremonyWorkCandidate;
use super::ceremony_worker_claim_error::CeremonyWorkerClaimError;
use super::{
    CeremonyWorkerAdmissionObserver, CeremonyWorkerAdmissionPolicy, CeremonyWorkerAdmissionReason,
    CeremonyWorkerEligibility, CeremonyWorkerRootPolicyPort, CeremonyWorkerScheduleRequest,
    CeremonyWorkerScheduler, WorkerAuthorizationError, WorkerAuthorizationPort,
    WorkerAuthorizationTarget, WorkerCapacityPort, WorkerCapacityRequest,
};
use made_core::value_objects::{ExecutionConnectorId, ExecutionOperationId, StepLease};
use std::sync::{Arc, Mutex};

use made_core::error::DomainError;
use made_core::ports::{
    BudgetReservationPlannerPort, CeremonyInstanceIndexPort, CeremonyStepHandlerRequest, ClockPort,
};
use made_core::value_objects::{
    BudgetReservationRequest, CeremonyId, CeremonyStep, IdempotencyKey, StepId,
};

use super::{
    CeremonyWorkClaimFailure, CeremonyWorkClaimsPage, CeremonyWorkerPolicy, ClaimCeremonyWorkInput,
    ExecuteCeremonyOperationInput,
};
use crate::budgets::{BudgetedStepClaimInput, BudgetedStepClaimUseCase};
use crate::services::AuthorizationOperationScope;
use crate::services::{ceremony_transcript_projection, SessionStream};
use crate::usecases::{
    CeremonyInstanceView, EnforceCeremonyDeadlinesInput, EnforceCeremonyDeadlinesUseCase,
    ResolveCeremonyDefinitionUseCase, StartCeremonyStepInput, StartCeremonyStepOutput,
    StartCeremonyStepUseCase,
};

/// Discovers ceremonies in keyset order and claims at most one external step per ceremony.
pub struct ClaimCeremonyWorkUseCase {
    index: Arc<dyn CeremonyInstanceIndexPort>,
    stream: Arc<SessionStream>,
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    deadlines: Arc<EnforceCeremonyDeadlinesUseCase>,
    start_step: Arc<StartCeremonyStepUseCase>,
    budgeted_step: Option<Arc<BudgetedStepClaimUseCase>>,
    budget_planner: Option<Arc<dyn BudgetReservationPlannerPort>>,
    clock: Arc<dyn ClockPort>,
    policy: CeremonyWorkerPolicy,
    scheduler: Mutex<CeremonyWorkerScheduler>,
    capacity: Option<(Arc<dyn WorkerCapacityPort>, ExecutionConnectorId)>,
    authorization: Option<Arc<dyn WorkerAuthorizationPort>>,
    admission: CeremonyWorkerAdmissionPolicy,
    root_policy: Option<Arc<dyn CeremonyWorkerRootPolicyPort>>,
}

impl std::fmt::Debug for ClaimCeremonyWorkUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClaimCeremonyWorkUseCase")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl ClaimCeremonyWorkUseCase {
    #[must_use]
    pub fn new(
        index: Arc<dyn CeremonyInstanceIndexPort>,
        stream: Arc<SessionStream>,
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        deadlines: Arc<EnforceCeremonyDeadlinesUseCase>,
        start_step: Arc<StartCeremonyStepUseCase>,
        clock: Arc<dyn ClockPort>,
        policy: CeremonyWorkerPolicy,
    ) -> Self {
        let admission = CeremonyWorkerAdmissionPolicy::new(
            super::CeremonyWorkerSchedulerPolicy::new(
                policy.max_parallel(),
                super::CeremonyWorkerCapacity::new(1).expect("one is valid capacity"),
                super::CeremonyWorkerPolicyVersion::new(1).expect("one is valid version"),
            ),
            super::CeremonyWorkerPriority::DEFAULT,
            super::CeremonyWorkerWeight::new(1).expect("one is valid weight"),
            super::CeremonyWorkerCost::new(1).expect("one is valid cost"),
            super::CeremonyWorkerCapacity::new(1).expect("one is valid capacity"),
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
            scheduler: Mutex::new(CeremonyWorkerScheduler::new(admission.scheduler())),
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
    pub fn with_admission_policy(mut self, admission: CeremonyWorkerAdmissionPolicy) -> Self {
        self.scheduler = Mutex::new(CeremonyWorkerScheduler::new(admission.scheduler()));
        self.admission = admission;
        self
    }

    #[must_use]
    pub fn with_admission_observer(
        mut self,
        observer: Arc<dyn CeremonyWorkerAdmissionObserver>,
    ) -> Self {
        self.scheduler = Mutex::new(CeremonyWorkerScheduler::with_observer(
            self.admission.scheduler(),
            observer,
        ));
        self
    }

    #[must_use]
    pub fn with_root_policy(mut self, policy: Arc<dyn CeremonyWorkerRootPolicyPort>) -> Self {
        self.root_policy = Some(policy);
        self
    }

    pub async fn execute(
        &self,
        input: ClaimCeremonyWorkInput,
    ) -> Result<CeremonyWorkClaimsPage, DomainError> {
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
            CeremonyWorkerScheduleRequest::new(
                candidate.root.clone(),
                candidate.operation.clone(),
                policy.priority(),
                policy.weight(),
                policy.cost(),
                policy.requested_capacity(),
                CeremonyWorkerEligibility::Ready,
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
                Err(error) => failures.push(CeremonyWorkClaimFailure::new(
                    candidate.ceremony.clone(),
                    error.into_domain(),
                )),
            }
        }
        Ok(CeremonyWorkClaimsPage::new(claims, failures, next_cursor))
    }

    async fn admit_candidate(
        &self,
        candidate: &CeremonyWorkCandidate,
        request: &CeremonyWorkerScheduleRequest,
        input: &ClaimCeremonyWorkInput,
    ) -> Result<Option<ExecuteCeremonyOperationInput>, CeremonyWorkerClaimError> {
        if !self.reserve_capacity(candidate, input).await? {
            self.observe(request.clone(), CeremonyWorkerAdmissionReason::Capacity)?;
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
                        CeremonyWorkerClaimError::Permission(_) => {
                            Some(CeremonyWorkerAdmissionReason::Permission)
                        }
                        CeremonyWorkerClaimError::Budget(_) => {
                            Some(CeremonyWorkerAdmissionReason::Budget)
                        }
                        CeremonyWorkerClaimError::Failure(_) => None,
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
        candidate: &CeremonyWorkCandidate,
        input: &ClaimCeremonyWorkInput,
    ) -> Result<bool, DomainError> {
        let Some((capacity, connector)) = &self.capacity else {
            return Ok(true);
        };
        let now = self.clock.now();
        let request = WorkerCapacityRequest {
            operation_id: candidate.operation.clone(),
            ceremony_id: candidate.ceremony.clone(),
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
        request: CeremonyWorkerScheduleRequest,
        reason: CeremonyWorkerAdmissionReason,
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
        ceremony_ids: &[CeremonyId],
    ) -> (Vec<CeremonyWorkCandidate>, Vec<CeremonyWorkClaimFailure>) {
        let mut candidates = Vec::new();
        let mut failures = Vec::new();
        for ceremony_id in ceremony_ids {
            let result = if let Some(authorization) = &self.authorization {
                let target = WorkerAuthorizationTarget::EnforceDeadline {
                    ceremony: ceremony_id.clone(),
                };
                let operation = authorization.authorize(&target).await;
                match operation {
                    Ok(operation) => {
                        Box::pin(AuthorizationOperationScope::run(
                            operation,
                            self.discover_one(ceremony_id),
                        ))
                        .await
                    }
                    Err(error) => Err(error.into_domain()),
                }
            } else {
                self.discover_one(ceremony_id).await
            };
            match result {
                Ok(Some(candidate)) => candidates.push(candidate),
                Ok(None) => {}
                Err(error) => {
                    failures.push(CeremonyWorkClaimFailure::new(ceremony_id.clone(), error));
                }
            }
        }
        (candidates, failures)
    }

    async fn discover_one(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<Option<CeremonyWorkCandidate>, DomainError> {
        let instance = self
            .deadlines
            .execute(EnforceCeremonyDeadlinesInput::new(ceremony_id.clone()))
            .await?;
        let definition = self.definitions.execute(&instance).await?;
        let view = CeremonyInstanceView::project_at(
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
                what: "ceremony_step",
            })?;
        let step = definition.step(&step_id).ok_or(DomainError::NotFound {
            what: "ceremony_step",
        })?;
        let provider = step
            .handler_config()
            .attributes()
            .get("provider")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(step.handler_kind().as_str());
        Ok(Some(CeremonyWorkCandidate {
            ceremony: ceremony_id.clone(),
            root: instance
                .lineage()
                .map_or_else(|| ceremony_id.clone(), |lineage| lineage.root_id().clone()),
            operation: ExecutionOperationId::for_step(
                ceremony_id,
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
        candidate: &CeremonyWorkCandidate,
        input: &ClaimCeremonyWorkInput,
    ) -> Result<Option<ExecuteCeremonyOperationInput>, CeremonyWorkerClaimError> {
        let Some(authorization) = &self.authorization else {
            return self
                .claim_one(&candidate.ceremony, input, Some(&candidate.step))
                .await;
        };
        let target = WorkerAuthorizationTarget::Claim {
            ceremony: candidate.ceremony.clone(),
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
                    CeremonyWorkerClaimError::Permission(error)
                }
                WorkerAuthorizationError::Failure(error) => {
                    CeremonyWorkerClaimError::Failure(error)
                }
            })?;
        Box::pin(AuthorizationOperationScope::run(
            operation,
            self.claim_one(&candidate.ceremony, input, Some(&candidate.step)),
        ))
        .await
    }

    fn connector_accepts(&self, step: &CeremonyStep) -> bool {
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
        ceremony_id: &CeremonyId,
        input: &ClaimCeremonyWorkInput,
        selected_step: Option<&StepId>,
    ) -> Result<Option<ExecuteCeremonyOperationInput>, CeremonyWorkerClaimError> {
        let instance = self
            .deadlines
            .execute(EnforceCeremonyDeadlinesInput::new(ceremony_id.clone()))
            .await?;
        let definition = self.definitions.execute(&instance).await?;
        let view = CeremonyInstanceView::project_at(
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
            ceremony_transcript_projection::transcript(&self.stream.records(ceremony_id).await?);
        let role_id = definition.role_id_for_step(&step_id)?;
        let idempotency_key = Self::idempotency_key(&instance, &step_id)?;
        let step = definition.step(&step_id).ok_or(DomainError::NotFound {
            what: "ceremony_step",
        })?;
        let claim_input = StartCeremonyStepInput::new(
            ceremony_id.clone(),
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
                what: "ceremony_step",
            })?;
        let sealed_role = accepted
            .claimed_role()
            .cloned()
            .unwrap_or(definition.role_id_for_step(&step_id)?);
        let request = CeremonyStepHandlerRequest::new(
            ceremony_id.clone(),
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
        Ok(Some(ExecuteCeremonyOperationInput {
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
        instance: &made_core::entities::CeremonyInstance,
        step: &CeremonyStep,
        claim_input: StartCeremonyStepInput,
    ) -> Result<StartCeremonyStepOutput, CeremonyWorkerClaimError> {
        if instance.budget_account_id().is_none() {
            return self
                .start_step
                .execute(claim_input)
                .await
                .map_err(CeremonyWorkerClaimError::Failure);
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
            .map_err(CeremonyWorkerClaimError::Failure)?;
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
                    CeremonyWorkerClaimError::Budget(domain)
                } else {
                    CeremonyWorkerClaimError::Failure(domain)
                }
            })
            .map(|output| output.claim().clone())
    }

    fn idempotency_key(
        instance: &made_core::entities::CeremonyInstance,
        step_id: &StepId,
    ) -> Result<IdempotencyKey, DomainError> {
        let record = instance.step_record(step_id).ok_or(DomainError::NotFound {
            what: "ceremony_step",
        })?;
        IdempotencyKey::new(format!(
            "worker:{}:{}:{}:{}:{}:{}",
            instance.id(),
            step_id,
            record.state_visit().get(),
            record.state_iteration().get(),
            record.iteration().get(),
            record.attempt().get().saturating_add(1)
        ))
    }
}
