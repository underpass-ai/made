use std::sync::Arc;

use made_core::entities::BudgetLedger;
use made_core::ports::{
    BudgetAppendOutcome, BudgetLedgerSnapshot, BudgetLedgerStorePort, BudgetReservationPage,
    ClockPort,
};
use made_core::value_objects::{
    BudgetAccountId, BudgetBalance, BudgetLedgerVersion, BudgetLimits, BudgetOperationId,
    BudgetPageLimit, BudgetReconciliationId, BudgetReservationEstimate, BudgetReservationId,
    ExecutionReceipt, MeasuredBudgetQuantities,
};
use made_core::BudgetError;

use super::BudgetMutationOutcome;

const MAX_CONFLICT_RETRIES: usize = 16;

#[derive(Clone)]
pub struct BudgetLedgerService {
    store: Arc<dyn BudgetLedgerStorePort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for BudgetLedgerService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BudgetLedgerService")
            .finish_non_exhaustive()
    }
}

impl BudgetLedgerService {
    #[must_use]
    pub fn new(store: Arc<dyn BudgetLedgerStorePort>, clock: Arc<dyn ClockPort>) -> Self {
        Self { store, clock }
    }

    pub async fn open(
        &self,
        account: BudgetAccountId,
        limits: BudgetLimits,
    ) -> Result<BudgetMutationOutcome, BudgetError> {
        for _ in 0..MAX_CONFLICT_RETRIES {
            let snapshot = self.store.load(&account).await?;
            let ledger = snapshot
                .as_ref()
                .map_or_else(BudgetLedger::empty, |value| value.ledger.clone());
            let Some(event) =
                ledger.decide_open(account.clone(), limits.clone(), self.clock.now())?
            else {
                return Ok(existing(snapshot.as_ref()));
            };
            if let Some(outcome) = appended(
                self.store
                    .append(&account, ledger.version(), vec![event])
                    .await?,
            ) {
                return Ok(outcome);
            }
        }
        Err(BudgetError::Persistence(made_core::DomainError::Conflict {
            what: "budget_ledger",
        }))
    }

    pub async fn reserve(
        &self,
        account: &BudgetAccountId,
        operation: BudgetOperationId,
        estimate: BudgetReservationEstimate,
    ) -> Result<BudgetMutationOutcome, BudgetError> {
        for _ in 0..MAX_CONFLICT_RETRIES {
            let snapshot = self
                .store
                .load(account)
                .await?
                .ok_or(BudgetError::LedgerNotOpen)?;
            let Some(event) =
                snapshot
                    .ledger
                    .decide_reserve(operation.clone(), estimate, self.clock.now())?
            else {
                return Ok(BudgetMutationOutcome::Existing {
                    version: snapshot.version,
                });
            };
            if let Some(outcome) = appended(
                self.store
                    .append(account, snapshot.version, vec![event])
                    .await?,
            ) {
                return Ok(outcome);
            }
        }
        Err(BudgetError::Persistence(made_core::DomainError::Conflict {
            what: "budget_ledger",
        }))
    }

    pub async fn reconcile(
        &self,
        account: &BudgetAccountId,
        reservation: BudgetReservationId,
        reconciliation: BudgetReconciliationId,
        measured: MeasuredBudgetQuantities,
    ) -> Result<BudgetMutationOutcome, BudgetError> {
        for _ in 0..MAX_CONFLICT_RETRIES {
            let snapshot = self
                .store
                .load(account)
                .await?
                .ok_or(BudgetError::LedgerNotOpen)?;
            let Some(event) = snapshot.ledger.decide_reconcile(
                reservation.clone(),
                reconciliation.clone(),
                measured,
                self.clock.now(),
            )?
            else {
                return Ok(BudgetMutationOutcome::Existing {
                    version: snapshot.version,
                });
            };
            if let Some(outcome) = appended(
                self.store
                    .append(account, snapshot.version, vec![event])
                    .await?,
            ) {
                return Ok(outcome);
            }
        }
        Err(BudgetError::Persistence(made_core::DomainError::Conflict {
            what: "budget_ledger",
        }))
    }

    pub async fn report(&self, account: &BudgetAccountId) -> Result<BudgetBalance, BudgetError> {
        self.store
            .load(account)
            .await?
            .ok_or(BudgetError::LedgerNotOpen)?
            .ledger
            .balance()
    }

    pub async fn reconcile_receipt(
        &self,
        account: &BudgetAccountId,
        receipt: &ExecutionReceipt,
    ) -> Result<BudgetMutationOutcome, BudgetError> {
        let operation = BudgetOperationId::for_execution(receipt.operation_id());
        self.reconcile(
            account,
            BudgetReservationId::for_operation(account, &operation),
            BudgetReconciliationId::for_receipt(receipt.receipt_id()),
            receipt.budget_measurement(),
        )
        .await
    }

    pub async fn pending(
        &self,
        after: Option<&BudgetReservationId>,
        limit: BudgetPageLimit,
    ) -> Result<BudgetReservationPage, BudgetError> {
        self.store
            .pending(after, limit)
            .await
            .map_err(BudgetError::from)
    }
}

fn existing(snapshot: Option<&BudgetLedgerSnapshot>) -> BudgetMutationOutcome {
    BudgetMutationOutcome::Existing {
        version: snapshot.map_or(BudgetLedgerVersion::EMPTY, |value| value.version),
    }
}
fn appended(outcome: BudgetAppendOutcome) -> Option<BudgetMutationOutcome> {
    match outcome {
        BudgetAppendOutcome::Appended { version } => {
            Some(BudgetMutationOutcome::Applied { version })
        }
        BudgetAppendOutcome::Conflict { .. } => None,
    }
}
