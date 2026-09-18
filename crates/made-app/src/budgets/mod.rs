mod budget_ledger_service;
mod budget_mutation_outcome;
mod budgeted_step_claim_input;
mod budgeted_step_claim_output;
mod budgeted_step_claim_use_case;
mod start_budgeted_ceremony_input;
mod start_budgeted_ceremony_use_case;

pub use budget_ledger_service::BudgetLedgerService;
pub use budget_mutation_outcome::BudgetMutationOutcome;
pub use budgeted_step_claim_input::BudgetedStepClaimInput;
pub use budgeted_step_claim_output::BudgetedStepClaimOutput;
pub use budgeted_step_claim_use_case::BudgetedStepClaimUseCase;
pub use start_budgeted_ceremony_input::StartBudgetedCeremonyInput;
pub use start_budgeted_ceremony_use_case::StartBudgetedCeremonyUseCase;

pub(crate) fn budget_error_to_domain(error: made_core::BudgetError) -> made_core::DomainError {
    match error {
        made_core::BudgetError::Persistence(error) => error,
        made_core::BudgetError::LedgerNotOpen => made_core::DomainError::NotFound {
            what: "budget_ledger",
        },
        made_core::BudgetError::ReservationNotFound(_) => made_core::DomainError::NotFound {
            what: "budget_reservation",
        },
        made_core::BudgetError::ReservationConflict(_) => made_core::DomainError::Conflict {
            what: "budget_reservation",
        },
        made_core::BudgetError::ReconciliationConflict(_) => made_core::DomainError::Conflict {
            what: "budget_reconciliation",
        },
        made_core::BudgetError::Exhausted { .. } => made_core::DomainError::Conflict {
            what: "budget_exhausted",
        },
        made_core::BudgetError::MissingReservationEstimate(_) => {
            made_core::DomainError::InvariantViolated {
                reason: "limited budget dimension requires a reservation estimate",
            }
        }
    }
}
