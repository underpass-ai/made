//! Recover the accepted response for an exact fenced completion retry.

use made_core::entities::ceremony_commands::ApplyStepResult;
use made_core::entities::{CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;

use super::CompleteCeremonyStepInput;
use crate::services::{current_authorized_operation, SessionStream};

pub(super) async fn replay_step_completion(
    stream: &SessionStream,
    input: &CompleteCeremonyStepInput,
    definition: &CeremonyDefinition,
) -> Result<Option<CeremonyInstance>, DomainError> {
    let records = stream.records(&input.instance_id).await?;
    for (index, record) in records.iter().enumerate() {
        let (step_id, result, finished_at) = match record.event() {
            Some(CeremonyEvent::StepCompleted(event)) => {
                (&event.step_id, &event.result, event.finished_at)
            }
            Some(CeremonyEvent::StepFailed(event)) => {
                (&event.step_id, &event.result, event.finished_at)
            }
            _ => continue,
        };
        if step_id != &input.step_id {
            continue;
        }
        let before = SessionStream::fold_records(&records[..index])?;
        if !before
            .instance
            .step_claim_fence(step_id)
            .is_ok_and(|fence| fence == input.claim_fence)
        {
            continue;
        }
        if result != &input.result || record.actor().kind() != input.actor_kind {
            return Err(DomainError::InvariantViolated {
                reason: "completed step claim already has a different result or actor kind",
            });
        }
        if let Some(evidence) = record.authorization_evidence() {
            if !current_authorized_operation().is_some_and(|operation| {
                operation.evidence().principal_id() == evidence.principal_id()
            }) {
                return Err(DomainError::InvariantViolated {
                    reason: "completion retry requires the original authenticated principal",
                });
            }
        }
        // Re-decide at the accepted cut and time to recover the entire atomic
        // result, including context writes and a reopened state iteration.
        // Never include later claims or lifecycle changes in the retry reply.
        let expected = before.instance.decide(
            &CeremonyCommand::ApplyStepResult(ApplyStepResult {
                step_id: input.step_id.clone(),
                result: input.result.clone(),
                claim_fence: input.claim_fence.clone(),
                now: finished_at,
            }),
            definition,
        )?;
        let end = index + expected.len();
        if end > records.len()
            || expected
                .iter()
                .zip(&records[index..end])
                .any(|(event, actual)| actual.event() != Some(event))
        {
            return Err(DomainError::InvariantViolated {
                reason: "accepted step completion does not match its sealed event batch",
            });
        }
        return SessionStream::fold_records(&records[..end]).map(|session| Some(session.instance));
    }
    Ok(None)
}
