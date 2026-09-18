use crate::entities::ceremony_commands::StartStep;
use crate::entities::CeremonyInstance;
use crate::error::DomainError;
use crate::value_objects::{
    BudgetOperationId, BudgetReservationId, ExecutionOperationId, StepExecutionRecord, StepId,
};

impl CeremonyInstance {
    pub(super) fn require_budget_reservation(
        &self,
        step_id: &StepId,
        record: &StepExecutionRecord,
        command: &StartStep,
    ) -> Result<(), DomainError> {
        match (&self.budget_account_id, &command.budget_reservation_id) {
            (None, None) => Ok(()),
            (Some(account), Some(reservation)) => {
                let operation = ExecutionOperationId::for_step(
                    self.id(),
                    step_id,
                    self.current_state_visit,
                    self.current_state_iteration,
                    record.iteration(),
                );
                let budget_operation = BudgetOperationId::for_execution(&operation);
                let expected = BudgetReservationId::for_operation(account, &budget_operation);
                if reservation == &expected {
                    Ok(())
                } else {
                    Err(DomainError::InvariantViolated {
                        reason:
                            "step claim budget reservation does not match its execution identity",
                    })
                }
            }
            (Some(_), None) => Err(DomainError::InvariantViolated {
                reason: "budgeted ceremony steps require an admitted reservation",
            }),
            (None, Some(_)) => Err(DomainError::InvariantViolated {
                reason: "unbudgeted ceremony steps cannot name a budget reservation",
            }),
        }
    }
}
