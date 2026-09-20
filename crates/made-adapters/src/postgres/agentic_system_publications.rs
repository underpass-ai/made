//! Sealed agentic system designs on Postgres.

use async_trait::async_trait;
use made_core::entities::{AgenticSystemPublicationOutcome, PublishedAgenticSystem};
use made_core::error::DomainError;
use made_core::ports::AgenticSystemPublicationPort;
use made_core::value_objects::{AgenticSystemId, AgenticSystemRevision};
use sqlx::{PgPool, Row};

use super::ceremony_store::{decode, encode, sqlx_error};
use super::PostgresPool;

/// Revisions that can no longer change, shared by every replica.
#[derive(Debug, Clone)]
pub struct PostgresAgenticSystemPublications {
    pool: PostgresPool,
}

impl PostgresAgenticSystemPublications {
    #[must_use]
    pub const fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    fn inner(&self) -> &PgPool {
        self.pool.inner()
    }
}

#[async_trait]
impl AgenticSystemPublicationPort for PostgresAgenticSystemPublications {
    /// The slot is taken by an insert that does nothing when it is
    /// occupied, and the occupant is read back in the same
    /// transaction. Checked first and written after, two replicas
    /// could both find the slot free.
    async fn publish(
        &self,
        published: PublishedAgenticSystem,
    ) -> Result<AgenticSystemPublicationOutcome, DomainError> {
        let mut transaction = self
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin agentic system publication"))?;
        let revision = i64::try_from(published.revision().get()).unwrap_or(i64::MAX);
        let inserted = sqlx::query(
            "INSERT INTO agentic_system_publications (system_id, revision, digest, payload) \
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
        )
        .bind(published.id().as_str())
        .bind(revision)
        .bind(published.digest().to_hex())
        .bind(encode(&published, "encode published agentic system")?)
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "seal agentic system revision"))?;
        if inserted.rows_affected() == 1 {
            transaction
                .commit()
                .await
                .map_err(|error| sqlx_error(error, "commit agentic system publication"))?;
            return Ok(AgenticSystemPublicationOutcome::Published(published));
        }
        let row = sqlx::query(
            "SELECT payload FROM agentic_system_publications \
             WHERE system_id = $1 AND revision = $2",
        )
        .bind(published.id().as_str())
        .bind(revision)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "read sealed agentic system"))?;
        let occupant = payload(&row)?;
        Ok(if occupant.digest() == published.digest() {
            AgenticSystemPublicationOutcome::AlreadyPublished(occupant)
        } else {
            AgenticSystemPublicationOutcome::RevisionOccupied {
                published: occupant.digest(),
                offered: published.digest(),
            }
        })
    }

    async fn published(
        &self,
        id: &AgenticSystemId,
        revision: AgenticSystemRevision,
    ) -> Result<Option<PublishedAgenticSystem>, DomainError> {
        let row = sqlx::query(
            "SELECT payload FROM agentic_system_publications \
             WHERE system_id = $1 AND revision = $2",
        )
        .bind(id.as_str())
        .bind(i64::try_from(revision.get()).unwrap_or(i64::MAX))
        .fetch_optional(self.inner())
        .await
        .map_err(|error| sqlx_error(error, "read sealed agentic system"))?;
        row.as_ref().map(payload).transpose()
    }

    async fn catalogue(&self) -> Result<Vec<PublishedAgenticSystem>, DomainError> {
        sqlx::query("SELECT payload FROM agentic_system_publications ORDER BY system_id, revision")
            .fetch_all(self.inner())
            .await
            .map_err(|error| sqlx_error(error, "catalogue agentic systems"))?
            .iter()
            .map(payload)
            .collect()
    }
}

fn payload(row: &sqlx::postgres::PgRow) -> Result<PublishedAgenticSystem, DomainError> {
    let bytes: Vec<u8> = row
        .try_get("payload")
        .map_err(|error| sqlx_error(error, "read sealed agentic system payload"))?;
    decode(&bytes, "decode published agentic system")
}
