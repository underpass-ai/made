//! Integrator bindings on Postgres, for the clustered service.

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{BindOutcome, BindReplacement, IntegratorBindingPort};
use made_core::value_objects::{
    IntegratorBinding, IntegratorBindingId, IntegratorScope, LoopProgressMark,
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use time::OffsetDateTime;

use crate::delivery::StoredIntegratorBinding;

use super::ceremony_store::{decode, encode, sqlx_error};
use super::PostgresPool;

/// Who is driving each ceremony, shared by every replica.
#[derive(Debug, Clone)]
pub struct PostgresIntegratorBindings {
    pool: PostgresPool,
}

impl PostgresIntegratorBindings {
    #[must_use]
    pub const fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    fn inner(&self) -> &PgPool {
        self.pool.inner()
    }
}

#[async_trait]
impl IntegratorBindingPort for PostgresIntegratorBindings {
    /// The scope row is locked before the incumbent is read, so two
    /// replicas cannot both conclude that they took a ceremony over.
    async fn bind(
        &self,
        binding: IntegratorBinding,
        replacement: BindReplacement,
    ) -> Result<BindOutcome, DomainError> {
        let key = binding.scope_key().to_string();
        let mut transaction = self
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin integrator binding"))?;
        let stored = locked(&mut transaction, &key).await?;
        let (next, outcome) = stored.bind(binding, replacement);
        if let Some(next) = next {
            upsert(&mut transaction, &key, &next).await?;
            transaction
                .commit()
                .await
                .map_err(|error| sqlx_error(error, "commit integrator binding"))?;
        }
        Ok(outcome)
    }

    async fn current(
        &self,
        scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        Ok(self
            .stored(&scope.scope_key().to_string())
            .await?
            .current()
            .cloned())
    }

    /// A binding is revoked by identifier, which does not name its
    /// scope, so the scopes are walked. There is one row per scope and
    /// a handful in flight; a second index nothing else reads would be
    /// a second thing to keep true.
    async fn revoke(
        &self,
        id: &IntegratorBindingId,
        now: OffsetDateTime,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        let mut transaction = self
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin integrator revocation"))?;
        let rows = sqlx::query("SELECT scope_key, payload FROM integrator_bindings FOR UPDATE")
            .fetch_all(&mut *transaction)
            .await
            .map_err(|error| sqlx_error(error, "scan integrator bindings"))?;
        let mut found = None;
        for row in rows {
            let key: String = row
                .try_get("scope_key")
                .map_err(|error| sqlx_error(error, "read integrator scope key"))?;
            let stored = payload(&row)?;
            if stored.bindings().iter().any(|binding| binding.id() == id) {
                found = Some((key, stored));
                break;
            }
        }
        let Some((key, stored)) = found else {
            return Ok(None);
        };
        let (next, revoked) = stored.revoke(id, now);
        if let Some(next) = next {
            upsert(&mut transaction, &key, &next).await?;
            transaction
                .commit()
                .await
                .map_err(|error| sqlx_error(error, "commit integrator revocation"))?;
        }
        Ok(revoked)
    }

    async fn record_progress(
        &self,
        id: &IntegratorBindingId,
        progress: LoopProgressMark,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        let mut transaction = self
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin integrator loop progress"))?;
        let rows = sqlx::query("SELECT scope_key, payload FROM integrator_bindings FOR UPDATE")
            .fetch_all(&mut *transaction)
            .await
            .map_err(|error| sqlx_error(error, "scan integrator bindings"))?;
        let mut found = None;
        for row in rows {
            let key: String = row
                .try_get("scope_key")
                .map_err(|error| sqlx_error(error, "read integrator scope key"))?;
            let stored = payload(&row)?;
            if stored.bindings().iter().any(|binding| binding.id() == id) {
                found = Some((key, stored));
                break;
            }
        }
        let Some((key, stored)) = found else {
            return Ok(None);
        };
        let (next, observed) = stored.observing(id, progress);
        if let Some(next) = next {
            upsert(&mut transaction, &key, &next).await?;
            transaction
                .commit()
                .await
                .map_err(|error| sqlx_error(error, "commit integrator loop progress"))?;
        }
        Ok(observed)
    }

    async fn list(
        &self,
        scope: Option<&IntegratorScope>,
    ) -> Result<Vec<IntegratorBinding>, DomainError> {
        if let Some(scope) = scope {
            return Ok(self
                .stored(&scope.scope_key().to_string())
                .await?
                .bindings()
                .to_vec());
        }
        let rows = sqlx::query("SELECT payload FROM integrator_bindings ORDER BY scope_key")
            .fetch_all(self.inner())
            .await
            .map_err(|error| sqlx_error(error, "list integrator bindings"))?;
        let mut every = Vec::new();
        for row in rows {
            every.extend(payload(&row)?.bindings().to_vec());
        }
        Ok(every)
    }
}

impl PostgresIntegratorBindings {
    async fn stored(&self, key: &str) -> Result<StoredIntegratorBinding, DomainError> {
        let row = sqlx::query("SELECT payload FROM integrator_bindings WHERE scope_key = $1")
            .bind(key)
            .fetch_optional(self.inner())
            .await
            .map_err(|error| sqlx_error(error, "read integrator binding"))?;
        row.as_ref()
            .map(payload)
            .transpose()
            .map(|stored| stored.unwrap_or_else(StoredIntegratorBinding::new))
    }
}

async fn locked(
    transaction: &mut Transaction<'_, Postgres>,
    key: &str,
) -> Result<StoredIntegratorBinding, DomainError> {
    let row =
        sqlx::query("SELECT payload FROM integrator_bindings WHERE scope_key = $1 FOR UPDATE")
            .bind(key)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|error| sqlx_error(error, "lock integrator binding"))?;
    row.as_ref()
        .map(payload)
        .transpose()
        .map(|stored| stored.unwrap_or_else(StoredIntegratorBinding::new))
}

async fn upsert(
    transaction: &mut Transaction<'_, Postgres>,
    key: &str,
    stored: &StoredIntegratorBinding,
) -> Result<(), DomainError> {
    sqlx::query(
        "INSERT INTO integrator_bindings (scope_key, payload) VALUES ($1, $2) \
         ON CONFLICT (scope_key) DO UPDATE SET payload = EXCLUDED.payload",
    )
    .bind(key)
    .bind(encode(stored, "encode integrator binding")?)
    .execute(&mut **transaction)
    .await
    .map_err(|error| sqlx_error(error, "write integrator binding"))?;
    Ok(())
}

fn payload(row: &sqlx::postgres::PgRow) -> Result<StoredIntegratorBinding, DomainError> {
    let bytes: Vec<u8> = row
        .try_get("payload")
        .map_err(|error| sqlx_error(error, "read integrator binding payload"))?;
    decode(&bytes, "decode integrator binding")
}
