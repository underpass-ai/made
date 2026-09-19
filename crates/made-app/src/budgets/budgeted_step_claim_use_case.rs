use std::sync::Arc;

use made_core::entities::ceremony_commands::StartStep;
use made_core::entities::{CeremonyCommand, CeremonyEvent};
use made_core::ports::ClockPort;
use made_core::value_objects::{
    BudgetOperationId, BudgetReservationId, ExecutionOperationId, MaxParallel, StepLease,
};
use made_core::BudgetError;

use super::{
    BudgetLedgerService, BudgetMutationOutcome, BudgetedStepClaimInput, BudgetedStepClaimOutput,
};
use crate::services::{session_facts, ConflictPolicy, SessionStream};
use crate::usecases::{ResolveCeremonyDefinitionUseCase, StartCeremonyStepOutput};

pub struct BudgetedStepClaimUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
    budgets: BudgetLedgerService,
    max_parallel_ceiling: MaxParallel,
}

impl std::fmt::Debug for BudgetedStepClaimUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BudgetedStepClaimUseCase")
            .finish_non_exhaustive()
    }
}

impl BudgetedStepClaimUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
        budgets: BudgetLedgerService,
    ) -> Self {
        Self {
            definitions,
            stream,
            clock,
            budgets,
            max_parallel_ceiling: MaxParallel::SERVER_MAX,
        }
    }

    #[must_use]
    pub fn with_max_parallel_ceiling(mut self, ceiling: MaxParallel) -> Self {
        self.max_parallel_ceiling = ceiling;
        self
    }

    pub async fn execute(
        &self,
        input: BudgetedStepClaimInput,
    ) -> Result<BudgetedStepClaimOutput, BudgetError> {
        let loaded = self.stream.load(&input.claim.instance_id).await?;
        let definition = self.definitions.execute(&loaded.instance).await?;
        let account_id = loaded.instance.budget_account_id().cloned().ok_or(
            made_core::DomainError::InvariantViolated {
                reason: "budgeted claim requires a ceremony budget account",
            },
        )?;
        let record = loaded.instance.step_record(&input.claim.step_id).ok_or(
            made_core::DomainError::NotFound {
                what: "ceremony_step",
            },
        )?;
        let operation_id = ExecutionOperationId::for_step(
            loaded.instance.id(),
            &input.claim.step_id,
            loaded.instance.current_state_visit(),
            loaded.instance.current_state_iteration(),
            record.iteration(),
        );
        let budget_operation = BudgetOperationId::for_execution(&operation_id);
        let reservation_id = BudgetReservationId::for_operation(&account_id, &budget_operation);

        // This durable admission precedes the ceremony append. A losing claimant never releases
        // it: the winning claimant has the same operation identity and may already be using it.
        let reservation = self
            .budgets
            .reserve(&account_id, budget_operation, input.reservation)
            .await?;

        let now = self.clock.now();
        if matches!(reservation, BudgetMutationOutcome::Existing { .. }) {
            if let Some(claim) = self
                .exact_existing_claim(&loaded, &input, &reservation_id, now)
                .await?
            {
                return Ok(BudgetedStepClaimOutput::new(
                    claim,
                    account_id,
                    operation_id,
                    reservation_id,
                ));
            }
        }
        let command = CeremonyCommand::StartStep(StartStep {
            role_id: input.claim.requested_role_id(),
            step_id: input.claim.step_id.clone(),
            lease: StepLease::acquire(
                input.claim.lease_owner_id,
                input.claim.idempotency_key,
                now,
                input.claim.lease_ttl,
            )?,
            now,
            max_parallel_ceiling: self.max_parallel_ceiling,
            budget_reservation_id: Some(reservation_id.clone()),
        });
        let actor_kind = input.claim.role_kind;
        let step_id = input.claim.step_id;
        let accepted = self
            .stream
            .execute(loaded, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&command, &definition)?;
                let actor = session_facts::step_started_seat(&events, actor_kind)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await?;
        let accepted_record =
            accepted
                .instance
                .step_record(&step_id)
                .ok_or(made_core::DomainError::NotFound {
                    what: "ceremony_step",
                })?;
        let output = StartCeremonyStepOutput::new(
            accepted.instance.clone(),
            accepted_record.attempt(),
            accepted.instance.step_claim_fence(&step_id)?,
            accepted.version,
        );
        Ok(BudgetedStepClaimOutput::new(
            output,
            account_id,
            operation_id,
            reservation_id,
        ))
    }

    async fn exact_existing_claim(
        &self,
        loaded: &crate::services::LoadedSession,
        input: &BudgetedStepClaimInput,
        reservation_id: &BudgetReservationId,
        now: time::OffsetDateTime,
    ) -> Result<Option<StartCeremonyStepOutput>, BudgetError> {
        let current = loaded.instance.step_record(&input.claim.step_id).ok_or(
            made_core::DomainError::NotFound {
                what: "ceremony_step",
            },
        )?;
        let requested_ttl =
            time::Duration::milliseconds(i64::try_from(input.claim.lease_ttl.get()).map_err(
                |_| made_core::DomainError::OutOfRange {
                    field: "step_lease.ttl_ms",
                    value: input.claim.lease_ttl.get() as f64,
                    min: 0.0,
                    max: i64::MAX as f64,
                },
            )?);
        let records = self.stream.records(loaded.instance.id()).await?;
        let Some(position) = records.iter().position(|record| {
            let Some(CeremonyEvent::StepStarted(started)) = record.event() else {
                return false;
            };
            started.step_id == input.claim.step_id
                && started.state_visit() == current.state_visit()
                && started.state_iteration() == current.state_iteration()
                && started.iteration == current.iteration()
                && started.attempt == current.attempt()
                && started.lease.owner_id() == &input.claim.lease_owner_id
                && started.lease.idempotency_key() == &input.claim.idempotency_key
                && started.lease.expires_at() - started.lease.acquired_at() == requested_ttl
                && !started.lease.is_expired_at(now)
                && started.budget_reservation_id.as_ref() == Some(reservation_id)
        }) else {
            return Ok(None);
        };
        let accepted = crate::services::SessionStream::fold_records(&records[..=position])?;
        let claim_fence = accepted.instance.step_claim_fence(&input.claim.step_id)?;
        Ok(Some(StartCeremonyStepOutput::new(
            accepted.instance,
            current.attempt(),
            claim_fence,
            accepted.version,
        )))
    }
}
