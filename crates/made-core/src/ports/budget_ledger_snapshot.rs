use crate::entities::BudgetLedger;
use crate::value_objects::BudgetLedgerVersion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetLedgerSnapshot {
    pub version: BudgetLedgerVersion,
    pub ledger: BudgetLedger,
}
