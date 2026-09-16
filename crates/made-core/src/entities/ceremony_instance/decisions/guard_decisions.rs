use crate::entities::ceremony_commands::{ApproveGuard, DeferGuard};
use crate::entities::ceremony_events::{HumanApprovalRecorded, HumanDeferralRecorded};
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::{CeremonyGuardApproval, CeremonyGuardDeferral, GuardCondition};

impl CeremonyInstance {
    /// Approving is checked the way deferring is: the guard must exist
    /// and wait on a person, and the approver must be a seat at this
    /// table. Approving ahead of time is allowed — unlike a deferral,
    /// which answers a decision being asked for now, a person may
    /// settle a guard before the work leading up to it is finished.
    pub(super) fn decide_approve_guard(
        &self,
        command: &ApproveGuard,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot approve guards",
        )?;
        let guard = definition
            .guards()
            .get(&command.guard_name)
            .ok_or(DomainError::NotFound {
                what: "ceremony_guard",
            })?;
        if !matches!(guard.condition(), GuardCondition::HumanApproval) {
            return Err(DomainError::InvariantViolated {
                reason: "only human approval guards can be approved",
            });
        }
        self.require_declared_role(definition, &command.approved_by)?;
        // The fold marks the guard approved in the context; that it
        // can is checked here so a refusal never surfaces there.
        self.context
            .clone()
            .with_guard_approval(&command.guard_name)?;
        Ok(vec![CeremonyEvent::HumanApprovalRecorded(
            HumanApprovalRecorded {
                approval: CeremonyGuardApproval::record(
                    command.guard_name.clone(),
                    command.approved_by.clone(),
                    command.approved_by_kind,
                    command.now,
                ),
            },
        )])
    }

    /// A deferral answers a decision being asked for now: the guard
    /// must block a transition out of the current state, and must not
    /// already be approved.
    pub(super) fn decide_defer_guard(
        &self,
        command: &DeferGuard,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot defer guard decisions",
        )?;
        let guard = definition
            .guards()
            .get(&command.guard_name)
            .ok_or(DomainError::NotFound {
                what: "ceremony_guard",
            })?;
        if !matches!(guard.condition(), GuardCondition::HumanApproval) {
            return Err(DomainError::InvariantViolated {
                reason: "only human approval guards can be deferred",
            });
        }
        if self.context.is_guard_approved(&command.guard_name) {
            return Err(DomainError::InvariantViolated {
                reason: "approved human guards cannot be deferred",
            });
        }
        let is_currently_required = definition
            .available_transitions(&self.current_state)
            .any(|transition| transition.required_guards().contains(&command.guard_name));
        if !is_currently_required {
            return Err(DomainError::InvariantViolated {
                reason: "human guard is not required from the current state",
            });
        }

        self.require_declared_role(definition, &command.deferred_by)?;
        Ok(vec![CeremonyEvent::HumanDeferralRecorded(
            HumanDeferralRecorded {
                deferral: CeremonyGuardDeferral::record(
                    command.guard_name.clone(),
                    command.deferred_by.clone(),
                    command.deferred_by_kind,
                    command.content.clone(),
                    command.now,
                ),
            },
        )])
    }
}
