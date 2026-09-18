use crate::services::{session_facts, ConflictPolicy, LoadedSession};
use made_core::entities::ceremony_commands::{ApplyStepResult, StartStep};
use made_core::entities::{CeremonyCommand, CeremonyDefinition};
use made_core::error::DomainError;
use made_core::ports::CeremonyStepHandlerRequest;
use made_core::value_objects::{
    AuditActorKind, CeremonyTranscript, DurationMs, IdempotencyKey, LeaseOwnerId, StepErrorMessage,
    StepId, StepLease, StepResult,
};

use super::{
    claimed_step::ClaimedStep, executed_step::ExecutedStep, run_step_output::RunStepOutput,
    RunCeremonyUseCase,
};

impl RunCeremonyUseCase {
    #[tracing::instrument(
        name = "ceremony_step",
        skip_all,
        fields(
            ceremony_id = %session.instance.id(),
            ceremony_name = %session.instance.definition_name(),
            state_id = %session.instance.current_state(),
            step_id = %step_id,
            role_id = tracing::field::Empty,
            state_iteration = tracing::field::Empty,
            iteration = tracing::field::Empty,
            attempt = tracing::field::Empty,
            outcome = tracing::field::Empty,
            step_status = tracing::field::Empty,
            error_kind = tracing::field::Empty,
        )
    )]
    pub(super) async fn run_step(
        &self,
        definition: &CeremonyDefinition,
        session: LoadedSession,
        actor_kind: AuditActorKind,
        step_id: &StepId,
        lease_owner_id: &LeaseOwnerId,
        lease_ttl: DurationMs,
        trace_index: usize,
        transcript: CeremonyTranscript,
    ) -> Result<RunStepOutput, DomainError> {
        let result = match self
            .claim_step(
                definition,
                session,
                actor_kind,
                step_id,
                lease_owner_id,
                lease_ttl,
                trace_index,
                transcript,
            )
            .await
        {
            Ok(claimed) => match self.execute_claimed_handler(claimed).await {
                Ok(executed) => {
                    self.complete_executed_step(definition, executed, actor_kind)
                        .await
                }
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };
        match &result {
            Ok(RunStepOutput {
                state_iteration,
                iteration,
                attempt,
                result: step_result,
                ..
            }) => {
                crate::usecases::step_span::record_coordinates(
                    *state_iteration,
                    *iteration,
                    *attempt,
                );
                crate::usecases::step_span::record_result(step_result);
            }
            Err(error) => crate::usecases::step_span::record_error(error),
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn claim_step(
        &self,
        definition: &CeremonyDefinition,
        session: LoadedSession,
        actor_kind: AuditActorKind,
        step_id: &StepId,
        lease_owner_id: &LeaseOwnerId,
        lease_ttl: DurationMs,
        trace_index: usize,
        transcript: CeremonyTranscript,
    ) -> Result<ClaimedStep, DomainError> {
        let step = definition
            .step(step_id)
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "ceremony_step",
            })?;
        let now = self.clock.now();
        let lease = StepLease::acquire(
            lease_owner_id.clone(),
            IdempotencyKey::new(format!(
                "{}:{}:{}",
                session.instance.id().as_str(),
                step_id.as_str(),
                trace_index + 1
            ))?,
            now,
            lease_ttl,
        )?;
        let claim = CeremonyCommand::StartStep(StartStep {
            role_id: None,
            step_id: step_id.clone(),
            lease,
            now,
            max_parallel_ceiling: self.max_parallel_ceiling,
        });
        // Appended before the handler runs, for the reason the step
        // use case appends twice: a crash while it runs must leave a
        // stream saying somebody took this step and never came back.
        let session = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&claim, definition)?;
                let actor = session_facts::step_started_seat(&events, actor_kind)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await?;
        // Captured off the claim, before the result is applied: a
        // successful repeat advances the record to the next iteration.
        let record = session
            .instance
            .step_record(step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_step",
            })?;
        let (state_visit, state_iteration, iteration, attempt) = (
            record.state_visit(),
            record.state_iteration(),
            record.iteration(),
            record.attempt(),
        );
        let claim_fence = session.instance.step_claim_fence(step_id)?;
        let sealed_role = record
            .claimed_role()
            .cloned()
            .map_or_else(|| definition.role_id_for_step(step_id), Ok)?;
        tracing::Span::current().record("role_id", sealed_role.as_str());

        let request = CeremonyStepHandlerRequest::new(
            session.instance.id().clone(),
            session.instance.definition_name().clone(),
            session.instance.definition_version().clone(),
            session.instance.current_state().clone(),
            step.id().clone(),
            step.handler_kind().clone(),
            step.handler_config().clone(),
            session.instance.context().clone(),
            attempt,
        )
        .with_transcript(transcript)
        .with_role(sealed_role.clone())
        .with_bound_specialty(session.instance.bound_specialty(&sealed_role).cloned());

        Ok(ClaimedStep {
            request,
            step_id: step_id.clone(),
            role_id: sealed_role,
            claim_fence,
            state_visit,
            state_iteration,
            iteration,
            attempt,
        })
    }

    pub(super) async fn execute_claimed_handler(
        &self,
        claimed: ClaimedStep,
    ) -> Result<ExecutedStep, DomainError> {
        let ClaimedStep {
            request,
            step_id,
            role_id,
            claim_fence,
            state_visit,
            state_iteration,
            iteration,
            attempt,
        } = claimed;
        let instance_id = request.instance_id().clone();
        let step_result = self.execute_handler(request).await?;

        Ok(ExecutedStep {
            instance_id,
            step_id,
            role_id,
            claim_fence,
            state_visit,
            state_iteration,
            iteration,
            attempt,
            result: step_result,
        })
    }

    pub(super) async fn complete_executed_step(
        &self,
        definition: &CeremonyDefinition,
        executed: ExecutedStep,
        actor_kind: AuditActorKind,
    ) -> Result<RunStepOutput, DomainError> {
        let ExecutedStep {
            instance_id,
            step_id,
            role_id,
            claim_fence,
            state_visit,
            state_iteration,
            iteration,
            attempt,
            result: step_result,
        } = executed;

        // Loaded again rather than reusing what the claim left: the
        // handler may have taken a while, and the version that was
        // current then is not the one this append has to expect.
        let finished_session = self.stream.load(&instance_id).await?;
        let finished_at = self.clock.now();
        let finish = CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step_id.clone(),
            result: step_result.clone(),
            claim_fence,
            now: finished_at,
        });
        let finish_actor_kind = actor_kind;
        let session = self
            .stream
            .execute(finished_session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&finish, definition)?;
                let finish_actor = session_facts::step_result_seat(&events, finish_actor_kind)?;
                session_facts::facts(&session.instance, events, &finish_actor, finished_at)
            })
            .await?;

        Ok(RunStepOutput {
            session,
            step_id,
            role_id,
            state_visit,
            state_iteration,
            iteration,
            attempt,
            result: step_result,
        })
    }

    #[tracing::instrument(
        name = "ceremony_step_handler",
        skip_all,
        fields(
            ceremony_id = %request.instance_id(),
            step_id = %request.step_id(),
            handler_kind = %request.handler_kind(),
            attempt = tracing::field::Empty,
            outcome = tracing::field::Empty,
            step_status = tracing::field::Empty,
            error_kind = tracing::field::Empty,
        )
    )]
    async fn execute_handler(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        crate::usecases::step_span::record_attempt(request.attempt());
        match self.handler.execute(request).await {
            Ok(result) => {
                crate::usecases::step_span::record_result(&result);
                Ok(result)
            }
            Err(error) => {
                crate::usecases::step_span::record_error(&error);
                let message = StepErrorMessage::new(error.to_string())?;
                let result = StepResult::failed(message)?;
                crate::usecases::step_span::record_status(&result);
                Ok(result)
            }
        }
    }
}
