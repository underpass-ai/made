use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{BudgetLedger, BudgetLedgerEvent};
use made_core::ports::{
    BudgetAppendOutcome, BudgetLedgerSnapshot, BudgetLedgerStorePort, BudgetReservationPage,
};
use made_core::value_objects::{
    BudgetAccountId, BudgetLedgerVersion, BudgetPageLimit, BudgetReservationId,
};
use made_core::DomainError;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct InMemoryBudgetLedgerStore {
    streams: Arc<RwLock<BTreeMap<BudgetAccountId, Vec<BudgetLedgerEvent>>>>,
}

impl InMemoryBudgetLedgerStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl BudgetLedgerStorePort for InMemoryBudgetLedgerStore {
    async fn load(
        &self,
        account: &BudgetAccountId,
    ) -> Result<Option<BudgetLedgerSnapshot>, DomainError> {
        let streams = self.streams.read().await;
        streams
            .get(account)
            .map(|events| snapshot(events))
            .transpose()
    }

    async fn append(
        &self,
        account: &BudgetAccountId,
        expected: BudgetLedgerVersion,
        events: Vec<BudgetLedgerEvent>,
    ) -> Result<BudgetAppendOutcome, DomainError> {
        if events.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "budget_events",
            });
        }
        if events.iter().any(|event| event.account_id() != account) {
            return Err(DomainError::InvariantViolated {
                reason: "budget append contains an event for another account",
            });
        }
        let mut streams = self.streams.write().await;
        let stored = streams.entry(account.clone()).or_default();
        let actual = BudgetLedgerVersion::new(stored.len() as u64);
        if actual != expected {
            return Ok(BudgetAppendOutcome::Conflict { expected, actual });
        }
        let mut candidate = stored.clone();
        candidate.extend(events);
        BudgetLedger::rehydrate(&candidate).map_err(|error| budget_decode_error(&error))?;
        *stored = candidate;
        Ok(BudgetAppendOutcome::Appended {
            version: BudgetLedgerVersion::new(stored.len() as u64),
        })
    }

    async fn pending(
        &self,
        after: Option<&BudgetReservationId>,
        limit: BudgetPageLimit,
    ) -> Result<BudgetReservationPage, DomainError> {
        let streams = self.streams.read().await;
        let mut reservations = Vec::new();
        for events in streams.values() {
            let ledger =
                BudgetLedger::rehydrate(events).map_err(|error| budget_decode_error(&error))?;
            reservations.extend(
                ledger
                    .reservations()
                    .filter(|item| item.reconciliation().is_none())
                    .cloned(),
            );
        }
        reservations.sort_by(|left, right| left.id().cmp(right.id()));
        reservations.retain(|item| after.is_none_or(|cursor| item.id() > cursor));
        reservations.truncate(limit.value());
        Ok(BudgetReservationPage::new(reservations))
    }
}

fn snapshot(events: &[BudgetLedgerEvent]) -> Result<BudgetLedgerSnapshot, DomainError> {
    let ledger = BudgetLedger::rehydrate(events).map_err(|error| budget_decode_error(&error))?;
    Ok(BudgetLedgerSnapshot {
        version: ledger.version(),
        ledger,
    })
}

fn budget_decode_error(error: &made_core::BudgetError) -> DomainError {
    tracing::error!(%error, "stored budget ledger violates its domain contract");
    DomainError::InvariantViolated {
        reason: "stored budget ledger violates its domain contract",
    }
}
