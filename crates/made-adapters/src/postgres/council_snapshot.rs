use super::council_journal_store::{append, begin};
use super::error::{serde_to_domain, sqlx_to_domain};
use super::PostgresPool;
use crate::council_data_snapshot::CouncilDataSnapshot;
use made_core::entities::{CouncilJournalEvent, CouncilJournalRecord, CouncilSnapshotProvenance};
use made_core::error::DomainError;
use made_core::value_objects::AuthorizationEvidence;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};

#[derive(Debug, Clone)]
pub struct PostgresCouncilSnapshot {
    pool: PostgresPool,
}
impl PostgresCouncilSnapshot {
    #[must_use]
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }
    pub async fn export(
        &self,
        provenance: CouncilSnapshotProvenance,
    ) -> Result<CouncilDataSnapshot, DomainError> {
        let mut tx = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|e| sqlx_to_domain(e, "export councils"))?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(|e| sqlx_to_domain(e, "export council isolation"))?;
        let snapshot = read(&mut tx, provenance).await?;
        snapshot.encode()?;
        Ok(snapshot)
    }
    pub async fn import(
        &self,
        snapshot: CouncilDataSnapshot,
    ) -> Result<CouncilJournalRecord, DomainError> {
        self.import_authorized(snapshot, None).await
    }
    pub async fn import_authorized(
        &self,
        snapshot: CouncilDataSnapshot,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<CouncilJournalRecord, DomainError> {
        let snapshot = snapshot.canonicalized();
        snapshot.encode()?;
        let mut tx = begin(&self.pool).await?;
        let source = snapshot.provenance.source().as_str();
        let prior =
            sqlx::query("SELECT snapshot, record FROM council_snapshot_imports WHERE source = $1")
                .bind(source)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| sqlx_to_domain(e, "read council import"))?;
        if let Some(prior) = prior {
            let original: CouncilDataSnapshot = decode(
                prior
                    .try_get("snapshot")
                    .map_err(|e| sqlx_to_domain(e, "read import snapshot"))?,
            )?;
            if original != snapshot {
                return Err(conflict());
            }
            return decode(
                prior
                    .try_get("record")
                    .map_err(|e| sqlx_to_domain(e, "read import record"))?,
            );
        }
        let existing = read(&mut tx, snapshot.provenance.clone()).await?;
        if !existing.is_empty() && existing != snapshot {
            return Err(conflict());
        }
        if existing.is_empty() {
            super::council_snapshot_write::write(&mut tx, &snapshot).await?;
        }
        let record = append(
            &mut tx,
            CouncilJournalEvent::SnapshotImported(snapshot.provenance.clone()),
            authorization,
        )
        .await?;
        sqlx::query(
            "INSERT INTO council_snapshot_imports (source, snapshot, record) VALUES ($1, $2, $3)",
        )
        .bind(source)
        .bind(encode(&snapshot)?)
        .bind(encode(&record)?)
        .execute(&mut *tx)
        .await
        .map_err(|e| sqlx_to_domain(e, "seal council import"))?;
        tx.commit()
            .await
            .map_err(|e| sqlx_to_domain(e, "commit council import"))?;
        Ok(record)
    }
}
fn conflict() -> DomainError {
    DomainError::InvariantViolated {
        reason: "council snapshot conflicts with existing data or provenance",
    }
}
pub(super) fn encode(value: &impl Serialize) -> Result<Value, DomainError> {
    serde_json::to_value(value).map_err(|e| serde_to_domain(&e, "encode council snapshot"))
}
fn decode<T: DeserializeOwned>(value: Value) -> Result<T, DomainError> {
    serde_json::from_value(value).map_err(|e| serde_to_domain(&e, "decode council snapshot"))
}
async fn list<T: DeserializeOwned>(
    tx: &mut Transaction<'_, Postgres>,
    query: &'static str,
) -> Result<Vec<T>, DomainError> {
    let rows: Vec<Value> = sqlx::query_scalar(query)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| sqlx_to_domain(e, "export council rows"))?;
    rows.into_iter().map(decode).collect()
}
async fn read(
    tx: &mut Transaction<'_, Postgres>,
    provenance: CouncilSnapshotProvenance,
) -> Result<CouncilDataSnapshot, DomainError> {
    Ok(CouncilDataSnapshot {
        schema_version: 1, provenance,
        councils: list(tx, "SELECT body FROM councils ORDER BY specialty").await?,
        agents: list(tx, "SELECT jsonb_build_object('id', agent_id, 'specialty', specialty, 'kind', kind, 'attributes', attributes) FROM agents ORDER BY agent_id").await?,
        contracts: list(tx, "SELECT body FROM council_contracts ORDER BY contract_id").await?,
        deliberations: list(tx, "SELECT body FROM deliberations ORDER BY task_id").await?,
        statistics: super::statistics::read_snapshot(tx).await?,
    })
}
