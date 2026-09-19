use async_trait::async_trait;
use made_core::entities::{BudgetLedger, BudgetLedgerEvent};
use made_core::ports::{
    BudgetAppendOutcome, BudgetLedgerSnapshot, BudgetLedgerStorePort, BudgetReservationPage,
};
use made_core::value_objects::{
    BudgetAccountId, BudgetLedgerVersion, BudgetPageLimit, BudgetReservation, BudgetReservationId,
};
use made_core::DomainError;
use sqlx::{Postgres, Row, Transaction};

use super::ceremony_store::{decode, encode, i64_to_u64, sqlx_error, u64_to_i64};
use super::PostgresCeremonyStore;

async fn load_events(
    executor: &mut Transaction<'_, Postgres>,
    account: &BudgetAccountId,
) -> Result<Vec<BudgetLedgerEvent>, DomainError> {
    let rows = sqlx::query(
        "SELECT version, payload FROM ceremony_budget_ledger_events \
         WHERE account_id = $1 ORDER BY version",
    )
    .bind(account.as_str())
    .fetch_all(&mut **executor)
    .await
    .map_err(|error| sqlx_error(error, "load budget ledger events"))?;
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let version: i64 = row
                .try_get("version")
                .map_err(|error| sqlx_error(error, "decode budget event version"))?;
            if i64_to_u64(version)? != index as u64 + 1 {
                return Err(DomainError::InvariantViolated {
                    reason: "postgres: budget event versions are not contiguous",
                });
            }
            let payload: Vec<u8> = row
                .try_get("payload")
                .map_err(|error| sqlx_error(error, "decode budget event payload"))?;
            let event: BudgetLedgerEvent = decode(&payload, "decode budget ledger event")?;
            if event.account_id() != account {
                return Err(DomainError::InvariantViolated {
                    reason: "postgres: budget event key does not match its payload",
                });
            }
            Ok(event)
        })
        .collect()
}

fn rehydrate(events: &[BudgetLedgerEvent]) -> Result<BudgetLedger, DomainError> {
    BudgetLedger::rehydrate(events).map_err(|error| {
        tracing::error!(%error, "stored Postgres budget ledger violates its contract");
        DomainError::InvariantViolated {
            reason: "stored Postgres budget ledger violates its domain contract",
        }
    })
}

async fn project_reservations(
    transaction: &mut Transaction<'_, Postgres>,
    account: &BudgetAccountId,
    ledger: &BudgetLedger,
) -> Result<(), DomainError> {
    for reservation in ledger.reservations() {
        sqlx::query(
            "INSERT INTO ceremony_budget_reservations \
             (reservation_id, account_id, pending, payload) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (reservation_id) DO UPDATE SET \
             account_id = EXCLUDED.account_id, pending = EXCLUDED.pending, \
             payload = EXCLUDED.payload",
        )
        .bind(reservation.id().as_str())
        .bind(account.as_str())
        .bind(reservation.reconciliation().is_none())
        .bind(encode(reservation, "encode budget reservation")?)
        .execute(&mut **transaction)
        .await
        .map_err(|error| sqlx_error(error, "project budget reservation"))?;
    }
    Ok(())
}

#[async_trait]
impl BudgetLedgerStorePort for PostgresCeremonyStore {
    async fn load(
        &self,
        account: &BudgetAccountId,
    ) -> Result<Option<BudgetLedgerSnapshot>, DomainError> {
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin budget ledger load"))?;
        // Head and events must describe one committed version even when a
        // different replica appends after the first SELECT.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(|error| sqlx_error(error, "isolate budget ledger snapshot"))?;
        let row = sqlx::query("SELECT version FROM ceremony_budget_ledgers WHERE account_id = $1")
            .bind(account.as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| sqlx_error(error, "load budget ledger head"))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let stored_version: i64 = row
            .try_get("version")
            .map_err(|error| sqlx_error(error, "decode budget ledger version"))?;
        let events = load_events(&mut transaction, account).await?;
        let ledger = rehydrate(&events)?;
        if ledger.version().value() != i64_to_u64(stored_version)? {
            return Err(DomainError::InvariantViolated {
                reason: "postgres: budget ledger head does not match its events",
            });
        }
        Ok(Some(BudgetLedgerSnapshot {
            version: ledger.version(),
            ledger,
        }))
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
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin budget ledger append"))?;
        sqlx::query(
            "INSERT INTO ceremony_budget_ledgers (account_id, version) VALUES ($1, 0) \
             ON CONFLICT (account_id) DO NOTHING",
        )
        .bind(account.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "create budget ledger"))?;
        let row = sqlx::query(
            "SELECT version FROM ceremony_budget_ledgers WHERE account_id = $1 FOR UPDATE",
        )
        .bind(account.as_str())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "lock budget ledger"))?;
        let actual = BudgetLedgerVersion::new(i64_to_u64(
            row.try_get("version")
                .map_err(|error| sqlx_error(error, "decode budget ledger version"))?,
        )?);
        if actual != expected {
            return Ok(BudgetAppendOutcome::Conflict { expected, actual });
        }
        let mut candidate = load_events(&mut transaction, account).await?;
        candidate.extend(events.iter().cloned());
        let ledger = rehydrate(&candidate)?;
        let mut version = expected;
        for event in events {
            version = version.next();
            sqlx::query(
                "INSERT INTO ceremony_budget_ledger_events \
                 (account_id, version, payload) VALUES ($1, $2, $3)",
            )
            .bind(account.as_str())
            .bind(u64_to_i64(version.value())?)
            .bind(encode(&event, "encode budget ledger event")?)
            .execute(&mut *transaction)
            .await
            .map_err(|error| sqlx_error(error, "insert budget ledger event"))?;
        }
        project_reservations(&mut transaction, account, &ledger).await?;
        sqlx::query("UPDATE ceremony_budget_ledgers SET version = $2 WHERE account_id = $1")
            .bind(account.as_str())
            .bind(u64_to_i64(version.value())?)
            .execute(&mut *transaction)
            .await
            .map_err(|error| sqlx_error(error, "advance budget ledger"))?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit budget ledger append"))?;
        Ok(BudgetAppendOutcome::Appended { version })
    }

    async fn pending(
        &self,
        after: Option<&BudgetReservationId>,
        limit: BudgetPageLimit,
    ) -> Result<BudgetReservationPage, DomainError> {
        let rows = sqlx::query(
            "SELECT reservation_id, payload FROM ceremony_budget_reservations \
             WHERE pending = TRUE AND reservation_id > $1 \
             ORDER BY reservation_id LIMIT $2",
        )
        .bind(after.map_or("", BudgetReservationId::as_str))
        .bind(i64::try_from(limit.value()).unwrap_or(i64::MAX))
        .fetch_all(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "list pending budget reservations"))?;
        let reservations = rows
            .into_iter()
            .map(|row| {
                let id: String = row
                    .try_get("reservation_id")
                    .map_err(|error| sqlx_error(error, "decode budget reservation id"))?;
                let payload: Vec<u8> = row
                    .try_get("payload")
                    .map_err(|error| sqlx_error(error, "decode budget reservation payload"))?;
                let reservation: BudgetReservation = decode(&payload, "decode budget reservation")?;
                if reservation.id().as_str() != id {
                    return Err(DomainError::InvariantViolated {
                        reason: "postgres: budget reservation key does not match its payload",
                    });
                }
                Ok(reservation)
            })
            .collect::<Result<Vec<_>, DomainError>>()?;
        Ok(BudgetReservationPage::new(reservations))
    }
}
