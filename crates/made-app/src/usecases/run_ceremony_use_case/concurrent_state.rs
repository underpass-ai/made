use std::sync::Arc;

use futures::future::join_all;
use made_core::entities::{CeremonyDefinition, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::CeremonyOutcome;
use made_core::value_objects::{AuditActorKind, DurationMs, LeaseOwnerId, StateId, StateIteration};
use tokio::sync::Semaphore;

use crate::services::{ceremony_transcript_projection, LoadedSession};

use super::{claimed_step::ClaimedStep, CeremonyStepTrace, RunCeremonyUseCase};

/// Fold and trace updates produced by one concurrent-state visit.
pub(super) struct ConcurrentStateOutput {
    pub(super) session: LoadedSession,
    pub(super) step_traces: Vec<CeremonyStepTrace>,
    pub(super) state_iteration_changed: bool,
    pub(super) step_failed: bool,
}

impl RunCeremonyUseCase {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn run_concurrent_state(
        &self,
        definition: &CeremonyDefinition,
        mut session: LoadedSession,
        actor_kind: AuditActorKind,
        state_id: &StateId,
        state_iteration: StateIteration,
        lease_owner_id: &LeaseOwnerId,
        lease_ttl: DurationMs,
        trace_offset: usize,
    ) -> Result<ConcurrentStateOutput, DomainError> {
        let width = usize::from(
            definition
                .max_parallel()
                .effective_with(self.max_parallel_ceiling)
                .get(),
        );
        let semaphore = Arc::new(Semaphore::new(width));
        let mut step_traces = Vec::new();
        let ceremony_id = session.instance.id().clone();

        loop {
            let now = self.clock.now();
            let claimable = session.instance.claimable_step_ids_at(
                definition,
                now,
                self.max_parallel_ceiling,
            )?;
            if claimable.is_empty() {
                break;
            }
            let step_ids = claimable
                .into_iter()
                .take(width)
                .cloned()
                .collect::<Vec<_>>();
            let transcript = ceremony_transcript_projection::transcript(
                &self.stream.records(session.instance.id()).await?,
            );

            // Claims are short, durable writes. Seal the whole bounded batch
            // before any handler runs so a crash cannot hide accepted work and
            // optimistic conflicts do not consume the batch's handler budget.
            let mut claimed = Vec::with_capacity(step_ids.len());
            let mut claim_error = None;
            for (index, step_id) in step_ids.iter().enumerate() {
                match self
                    .claim_step(
                        definition,
                        session,
                        actor_kind,
                        step_id,
                        lease_owner_id,
                        lease_ttl,
                        trace_offset + step_traces.len() + index,
                        transcript.clone(),
                    )
                    .await
                {
                    Ok(claim) => {
                        session = self.stream.load(&ceremony_id).await?;
                        claimed.push(claim);
                    }
                    Err(error) => {
                        claim_error = Some(error);
                        break;
                    }
                }
            }

            let (batch_traces, step_failed) = self
                .execute_claimed_batch(
                    definition,
                    actor_kind,
                    state_id,
                    claimed,
                    semaphore.clone(),
                    claim_error,
                )
                .await?;
            step_traces.extend(batch_traces);
            session = self.stream.load(&ceremony_id).await?;
            if step_failed || session.instance.current_state_iteration() != state_iteration {
                let state_iteration_changed =
                    session.instance.current_state_iteration() != state_iteration;
                return Ok(ConcurrentStateOutput {
                    session,
                    step_traces,
                    state_iteration_changed,
                    step_failed,
                });
            }
            self.refuse_exhausted_state_repeat(definition, &session.instance)?;

            // An any/count join may now be enabled. Let the outer driver apply
            // it before claiming another batch. No accepted work is cancelled:
            // this point is reached only after the whole batch was completed.
            if definition
                .available_transitions(state_id)
                .any(|transition| {
                    session
                        .instance
                        .transition_is_enabled(definition, transition)
                })
            {
                break;
            }
        }

        Ok(ConcurrentStateOutput {
            session,
            step_traces,
            state_iteration_changed: false,
            step_failed: false,
        })
    }

    fn refuse_exhausted_state_repeat(
        &self,
        definition: &CeremonyDefinition,
        instance: &CeremonyInstance,
    ) -> Result<(), DomainError> {
        if !instance.state_repeat_limit_reached(definition) {
            return Ok(());
        }
        self.metrics.record_ceremony_outcome(
            definition.name().as_str(),
            CeremonyOutcome::StateRepeatLimit,
        );
        Err(DomainError::InvariantViolated {
            reason: "ceremony state repeat limit exhausted",
        })
    }

    async fn execute_claimed_batch(
        &self,
        definition: &CeremonyDefinition,
        actor_kind: AuditActorKind,
        state_id: &StateId,
        claimed: Vec<ClaimedStep>,
        semaphore: Arc<Semaphore>,
        mut first_error: Option<DomainError>,
    ) -> Result<(Vec<CeremonyStepTrace>, bool), DomainError> {
        // join_all drains every accepted sibling even when one result is
        // failed or refused. Dropping the remaining futures would strand
        // durable InProgress records until their leases expired.
        let executions = join_all(claimed.into_iter().map(|claim| {
            let semaphore = semaphore.clone();
            async move {
                let _permit = semaphore.acquire_owned().await.map_err(|_| {
                    DomainError::InvariantViolated {
                        reason: "ceremony concurrency limiter closed",
                    }
                })?;
                self.execute_and_complete_claimed_step(definition, claim, actor_kind)
                    .await
            }
        }))
        .await;
        let mut step_failed = false;
        let mut traces = Vec::with_capacity(executions.len());
        for execution in executions {
            match execution {
                Ok(output) => {
                    step_failed |= !output.result.is_success();
                    traces.push(
                        CeremonyStepTrace::for_coordinates(
                            state_id.clone(),
                            output.state_iteration,
                            output.step_id,
                            output.role_id,
                            output.iteration,
                            output.attempt,
                            output.result.status(),
                            output.result.output().clone(),
                        )
                        .with_state_visit(output.state_visit),
                    );
                }
                Err(error) if first_error.is_none() => first_error = Some(error),
                Err(_) => {}
            }
        }
        first_error.map_or(Ok((traces, step_failed)), Err)
    }
}
