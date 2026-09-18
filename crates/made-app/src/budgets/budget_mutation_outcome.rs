use made_core::value_objects::BudgetLedgerVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetMutationOutcome {
    Applied { version: BudgetLedgerVersion },
    Existing { version: BudgetLedgerVersion },
}
