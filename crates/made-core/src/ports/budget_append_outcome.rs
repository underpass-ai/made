use crate::value_objects::BudgetLedgerVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetAppendOutcome {
    Appended {
        version: BudgetLedgerVersion,
    },
    Conflict {
        expected: BudgetLedgerVersion,
        actual: BudgetLedgerVersion,
    },
}
