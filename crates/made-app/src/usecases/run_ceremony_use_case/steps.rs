use crate::services::{session_facts, ConflictPolicy, LoadedSession};
use made_core::entities::ceremony_commands::{ApplyStepResult, StartStep};
use made_core::entities::{CeremonyCommand, CeremonyDefinition};
use made_core::error::DomainError;
use made_core::ports::CeremonyStepHandlerRequest;
use made_core::value_objects::{
    AuditActor, CeremonyTranscript, DurationMs, IdempotencyKey, LeaseOwnerId, RoleId, StepAttempt,
    StepErrorMessage, StepId, StepLease, StepResult,
};

use super::RunCeremonyUseCase;

impl RunCeremonyUseCase {
    pub(super) async fn run_step(
        &self,
        definition: &CeremonyDefinition,
        session: LoadedSession,
        role_id: &RoleId,
        actor: &AuditActor,
        step_id: &StepId,
        lease_owner_id: &LeaseOwnerId,
        lease_ttl: DurationMs,
        trace_index: usize,
        transcript: CeremonyTranscript,
    ) -> Result<
        (
            LoadedSession,
            made_core::value_objects::StepIteration,
            StepAttempt,
            StepResult,
        ),
        DomainError,
    > {
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
            role_id: Some(role_id.clone()),
            step_id: step_id.clone(),
            lease,
            now,
            max_parallel_ceiling: made_core::value_objects::MaxParallel::SERVER_MAX,
        });
        // Appended before the handler runs, for the reason the step
        // use case appends twice: a crash while it runs must leave a
        // stream saying somebody took this step and never came back.
        let session = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&claim, definition)?;
                session_facts::facts(&session.instance, events, actor, now)
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
        let (iteration, attempt) = (record.iteration(), record.attempt());

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
        .with_role(role_id.clone())
        .with_bound_specialty(session.instance.bound_specialty(role_id).cloned());
        let step_result = self.execute_handler(request).await?;

        // Loaded again rather than reusing what the claim left: the
        // handler may have taken a while, and the version that was
        // current then is not the one this append has to expect.
        let finished_session = self.stream.load(session.instance.id()).await?;
        let finished_at = self.clock.now();
        let finish = CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: step_id.clone(),
            result: step_result.clone(),
            now: finished_at,
        });
        let session = self
            .stream
            .execute(finished_session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&finish, definition)?;
                session_facts::facts(&session.instance, events, actor, finished_at)
            })
            .await?;

        Ok((session, iteration, attempt, step_result))
    }

    async fn execute_handler(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        match self.handler.execute(request).await {
            Ok(result) => Ok(result),
            Err(error) => {
                let message = StepErrorMessage::new(error.to_string())?;
                StepResult::failed(message)
            }
        }
    }
}
