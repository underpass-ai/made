use crate::entities::ceremony_commands::RecordHostHandoff;
use crate::entities::ceremony_events::HostHandoffRecorded;
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::value_objects::IdempotencyKey;
use crate::DomainError;

impl CeremonyInstance {
    #[must_use]
    pub fn host_handoffs(
        &self,
    ) -> &std::collections::BTreeMap<IdempotencyKey, HostHandoffRecorded> {
        &self.host_handoffs
    }

    pub(super) fn decide_record_host_handoff(
        &self,
        command: &RecordHostHandoff,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        let declaration = &command.declaration;
        declaration.validate()?;
        if let Some(accepted) = self.host_handoffs.get(&declaration.id) {
            return if accepted.declaration == *declaration {
                // A durable receipt is replayable even after the producer fence is
                // retired. Returning no event cannot renew, revive, or transfer it.
                Ok(Vec::new())
            } else {
                Err(DomainError::Conflict {
                    what: "host_handoff_identity",
                })
            };
        }
        // A new declaration must name the exact current producer. This check is
        // deliberately after identity replay: a retried durable receipt is a read,
        // whereas accepting another declaration changes the journal.
        self.require_step_claim_fence(&declaration.step_id, &declaration.claim_fence)?;
        let lease = self
            .step_record(&declaration.step_id)
            .and_then(|record| record.lease())
            .ok_or(DomainError::NotFound {
                what: "accepted_step_lease",
            })?;
        if lease.owner_id() != &declaration.owner
            || declaration.observed_at < lease.acquired_at()
            || declaration.observed_at > command.now
        {
            return Err(DomainError::InvariantViolated {
                reason: "host handoff must name its accepted owner and a valid observation time",
            });
        }
        if self.host_handoffs.values().any(|accepted| {
            accepted.declaration.claim_fence == declaration.claim_fence
                && (accepted.declaration.incarnation != declaration.incarnation
                    || accepted.declaration.observed_at >= declaration.observed_at)
        }) {
            return Err(DomainError::Conflict {
                what: "host_handoff_incarnation_or_order",
            });
        }
        Ok(vec![CeremonyEvent::HostHandoffRecorded(
            HostHandoffRecorded {
                declaration: declaration.clone(),
                recorded_at: command.now,
            },
        )])
    }
}
