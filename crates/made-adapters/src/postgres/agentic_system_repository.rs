//! Agentic system designs on Postgres, for the clustered service.

use async_trait::async_trait;
use made_core::entities::AgenticSystem;
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemPage, AgenticSystemQuery, AgenticSystemRepositoryPort, AgenticSystemSaveOutcome,
};
use made_core::value_objects::{AgenticSystemId, AgenticSystemLifecycle, AgenticSystemRevision};
use sqlx::{PgPool, Row};

use super::ceremony_store::{decode, encode, sqlx_error};
use super::PostgresPool;

/// The revision log of designs, shared by every replica.
#[derive(Debug, Clone)]
pub struct PostgresAgenticSystemRepository {
    pool: PostgresPool,
}

impl PostgresAgenticSystemRepository {
    #[must_use]
    pub const fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    fn inner(&self) -> &PgPool {
        self.pool.inner()
    }
}

#[async_trait]
impl AgenticSystemRepositoryPort for PostgresAgenticSystemRepository {
    /// The head is read under a row lock and the next revision
    /// inserted in the same transaction. Two replicas that both read
    /// revision 3 would otherwise both write revision 4, and the
    /// second would replace an edit it had never seen.
    async fn save(
        &self,
        system: AgenticSystem,
        expected: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemSaveOutcome, DomainError> {
        let mut transaction = self
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin agentic system save"))?;
        let head: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(revision) FROM agentic_systems WHERE system_id = $1 FOR UPDATE",
        )
        .bind(system.id().as_str())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "lock agentic system head"))?;
        let head = head
            .map(|revision| AgenticSystemRevision::new(revision.unsigned_abs()))
            .transpose()?;
        if head != expected {
            return Ok(AgenticSystemSaveOutcome::conflict(
                head.unwrap_or(AgenticSystemRevision::INITIAL),
            ));
        }
        let revision = head.map_or(AgenticSystemRevision::INITIAL, AgenticSystemRevision::next);
        let stored = system.at_revision(revision);
        sqlx::query(
            "INSERT INTO agentic_systems (system_id, revision, lifecycle, payload) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(stored.id().as_str())
        .bind(i64::try_from(revision.get()).unwrap_or(i64::MAX))
        .bind(stored.lifecycle().as_str())
        .bind(encode(&stored, "encode agentic system")?)
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "insert agentic system revision"))?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit agentic system save"))?;
        Ok(AgenticSystemSaveOutcome::saved(revision))
    }

    async fn get(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
    ) -> Result<Option<AgenticSystem>, DomainError> {
        let row =
            match revision {
                Some(revision) => sqlx::query(
                    "SELECT payload FROM agentic_systems WHERE system_id = $1 AND revision = $2",
                )
                .bind(id.as_str())
                .bind(i64::try_from(revision.get()).unwrap_or(i64::MAX))
                .fetch_optional(self.inner())
                .await,
                None => {
                    sqlx::query(
                        "SELECT payload FROM agentic_systems WHERE system_id = $1 \
                 ORDER BY revision DESC LIMIT 1",
                    )
                    .bind(id.as_str())
                    .fetch_optional(self.inner())
                    .await
                }
            }
            .map_err(|error| sqlx_error(error, "read agentic system"))?;
        row.as_ref().map(payload).transpose()
    }

    /// Heads only: `DISTINCT ON` takes the largest revision of each
    /// design, which is what the descending index is there for.
    async fn list(&self, query: &AgenticSystemQuery) -> Result<AgenticSystemPage, DomainError> {
        let rows = sqlx::query(
            "SELECT payload FROM ( \
                SELECT DISTINCT ON (system_id) system_id, revision, lifecycle, payload \
                FROM agentic_systems \
                WHERE ($1::text IS NULL OR system_id > $1) \
                ORDER BY system_id, revision DESC \
             ) heads \
             WHERE ($2::text IS NULL OR lifecycle = $2) \
             ORDER BY system_id LIMIT $3",
        )
        .bind(query.after().map(AgenticSystemId::as_str))
        .bind(query.lifecycle().map(AgenticSystemLifecycle::as_str))
        // One more than asked for, so a full page can offer a cursor
        // without a second round trip to find out whether it needs one.
        .bind(i64::from(query.limit().get()) + 1)
        .fetch_all(self.inner())
        .await
        .map_err(|error| sqlx_error(error, "list agentic systems"))?;
        let mut systems = rows
            .iter()
            .map(payload)
            .collect::<Result<Vec<AgenticSystem>, _>>()?;
        let next_cursor = (systems.len() > query.limit().as_usize())
            .then(|| systems[query.limit().as_usize() - 1].id().clone());
        systems.truncate(query.limit().as_usize());
        Ok(AgenticSystemPage::new(systems, next_cursor))
    }
}

fn payload(row: &sqlx::postgres::PgRow) -> Result<AgenticSystem, DomainError> {
    let bytes: Vec<u8> = row
        .try_get("payload")
        .map_err(|error| sqlx_error(error, "read agentic system payload"))?;
    decode(&bytes, "decode agentic system")
}
