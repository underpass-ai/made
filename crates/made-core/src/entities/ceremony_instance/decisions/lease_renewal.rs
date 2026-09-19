use crate::entities::ceremony_commands::RenewStepLease;
use crate::entities::ceremony_events::StepLeaseRenewed;
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;

impl CeremonyInstance {
    pub(super) fn decide_renew_step_lease(
        &self,
        command: &RenewStepLease,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        if self.is_ended() || self.is_terminal(definition) {
            return Err(DomainError::InvariantViolated {
                reason: "ended ceremony cannot renew a lease",
            });
        }
        self.require_step_claim_fence(&command.step_id, &command.claim_fence)?;
        let record = self
            .step_record(&command.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_step",
            })?;
        let lease = record
            .lease()
            .ok_or(DomainError::NotFound { what: "step_lease" })?;
        if lease.owner_id() != &command.lease_owner_id || !record.has_live_lease_at(command.now) {
            return Err(DomainError::InvariantViolated {
                reason: "only the live lease owner may renew",
            });
        }
        if record.effective_lease_expires_at() != Some(command.expected_expires_at) {
            return Err(DomainError::Conflict {
                what: "step_lease_expiry",
            });
        }
        if command.expires_at < command.expected_expires_at || command.expires_at <= command.now {
            return Err(DomainError::InvariantViolated {
                reason: "renewal cannot shorten the effective lease expiry",
            });
        }
        let deadline = self
            .step_deadlines
            .get(&command.step_id)
            .map(crate::value_objects::StepDeadline::at)
            .into_iter()
            .chain(
                self.state_deadline
                    .as_ref()
                    .map(crate::value_objects::StateDeadline::at),
            )
            .chain(
                self.ceremony_deadline
                    .map(crate::value_objects::CeremonyDeadline::at),
            )
            .min();
        if deadline.is_some_and(|at| command.now >= at || command.expires_at > at) {
            return Err(DomainError::InvariantViolated {
                reason: "renewal cannot exceed the absolute deadline",
            });
        }
        if command.expires_at == command.expected_expires_at {
            return Ok(Vec::new());
        }
        Ok(vec![CeremonyEvent::StepLeaseRenewed(StepLeaseRenewed {
            step_id: command.step_id.clone(),
            claim_fence: command.claim_fence.clone(),
            lease_owner_id: command.lease_owner_id.clone(),
            previous_expires_at: command.expected_expires_at,
            expires_at: command.expires_at,
            renewed_at: command.now,
        })])
    }
}
