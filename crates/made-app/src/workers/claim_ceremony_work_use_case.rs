use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{BudgetReservationPlannerPort, CeremonyStepHandlerRequest, ClockPort};
use made_core::value_objects::{
    BudgetReservationRequest, CeremonyId, CeremonyStep, IdempotencyKey, StepId,
};

use super::{
    CeremonyWorkClaimFailure, CeremonyWorkClaimsPage, CeremonyWorkerPolicy, ClaimCeremonyWorkInput,
    ExecuteCeremonyOperationInput,
};
use crate::budgets::{BudgetedStepClaimInput, BudgetedStepClaimUseCase};
use crate::services::{ceremony_transcript_projection, SessionStream};
use crate::usecases::{
    CeremonyInstanceView, EnforceCeremonyDeadlinesInput, EnforceCeremonyDeadlinesUseCase,
    ResolveCeremonyDefinitionUseCase, StartCeremonyStepInput, StartCeremonyStepOutput,
    StartCeremonyStepUseCase,
};

/// Discovers ceremonies in keyset order and claims at most one external step per ceremony.
pub struct ClaimCeremonyWorkUseCase {
    stream: Arc<SessionStream>,
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    deadlines: Arc<EnforceCeremonyDeadlinesUseCase>,
    start_step: Arc<StartCeremonyStepUseCase>,
    budgeted_step: Option<Arc<BudgetedStepClaimUseCase>>,
    budget_planner: Option<Arc<dyn BudgetReservationPlannerPort>>,
    clock: Arc<dyn ClockPort>,
    policy: CeremonyWorkerPolicy,
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
        stream: Arc<SessionStream>,
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        deadlines: Arc<EnforceCeremonyDeadlinesUseCase>,
        start_step: Arc<StartCeremonyStepUseCase>,
        clock: Arc<dyn ClockPort>,
        policy: CeremonyWorkerPolicy,
    ) -> Self {
        Self {
            stream,
            definitions,
            deadlines,
            start_step,
            budgeted_step: None,
            budget_planner: None,
            clock,
            policy,
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

    pub async fn execute(
        &self,
        input: ClaimCeremonyWorkInput,
    ) -> Result<CeremonyWorkClaimsPage, DomainError> {
        let mut ids = self
            .stream
            .ids()
            .await?
            .into_iter()
            .filter(|id| input.after().is_none_or(|after| id > after))
            .take(usize::from(input.limit().get()) + 1)
            .collect::<Vec<_>>();
        let has_more = ids.len() > usize::from(input.limit().get());
        if has_more {
            ids.pop();
        }
        let next_cursor = has_more.then(|| ids.last().cloned()).flatten();
        let mut claims = Vec::new();
        let mut failures = Vec::new();
        for ceremony_id in ids {
            match self.claim_one(&ceremony_id, &input).await {
                Ok(Some(claim)) => claims.push(claim),
                Ok(None) => {}
                Err(error) => failures.push(CeremonyWorkClaimFailure::new(ceremony_id, error)),
            }
        }
        Ok(CeremonyWorkClaimsPage::new(claims, failures, next_cursor))
    }

    async fn claim_one(
        &self,
        ceremony_id: &CeremonyId,
        input: &ClaimCeremonyWorkInput,
    ) -> Result<Option<ExecuteCeremonyOperationInput>, DomainError> {
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
                definition
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
    ) -> Result<StartCeremonyStepOutput, DomainError> {
        if instance.budget_account_id().is_none() {
            return self.start_step.execute(claim_input).await;
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
            .await?;
        self.budgeted_step
            .as_ref()
            .ok_or(DomainError::InvariantViolated {
                reason: "budgeted worker claim requires a budget ledger",
            })?
            .execute(BudgetedStepClaimInput::new(claim_input, estimate))
            .await
            .map_err(crate::budgets::budget_error_to_domain)
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
