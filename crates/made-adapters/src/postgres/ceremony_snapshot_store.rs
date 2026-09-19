use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{CeremonySnapshot, CeremonySnapshotStorePort};
use made_core::value_objects::CeremonyId;
use sqlx::Row;

use super::ceremony_store::{decode, encode, i64_to_u64, sqlx_error, u64_to_i64};
use super::PostgresCeremonyStore;

#[async_trait]
impl CeremonySnapshotStorePort for PostgresCeremonyStore {
    async fn save(&self, snapshot: CeremonySnapshot) -> Result<(), DomainError> {
        let stream_id = snapshot.instance.id().as_str().to_owned();
        let version = snapshot.version.value();
        let payload = encode(
            &(snapshot.version, &snapshot.instance),
            "encode ceremony snapshot",
        )?;
        sqlx::query(
            "INSERT INTO ceremony_snapshots (stream_id, version, payload) VALUES ($1, $2, $3) \
             ON CONFLICT (stream_id, version) DO UPDATE SET payload = EXCLUDED.payload",
        )
        .bind(stream_id)
        .bind(u64_to_i64(version)?)
        .bind(payload)
        .execute(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "save ceremony snapshot"))?;
        Ok(())
    }

    async fn latest(&self, stream: &CeremonyId) -> Result<Option<CeremonySnapshot>, DomainError> {
        let row = sqlx::query(
            "SELECT version, payload FROM ceremony_snapshots \
             WHERE stream_id = $1 ORDER BY version DESC LIMIT 1",
        )
        .bind(stream.as_str())
        .fetch_optional(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "read latest ceremony snapshot"))?;
        row.map(|row| {
            let stored_version: i64 = row
                .try_get("version")
                .map_err(|error| sqlx_error(error, "decode ceremony snapshot version"))?;
            let payload: Vec<u8> = row
                .try_get("payload")
                .map_err(|error| sqlx_error(error, "decode ceremony snapshot payload"))?;
            let (version, instance) = decode(&payload, "decode ceremony snapshot")?;
            let snapshot = CeremonySnapshot { version, instance };
            if snapshot.instance.id() != stream
                || snapshot.version.value() != i64_to_u64(stored_version)?
            {
                return Err(DomainError::InvariantViolated {
                    reason: "postgres: ceremony snapshot key does not match its payload",
                });
            }
            Ok(snapshot)
        })
        .transpose()
    }

    async fn forget(&self, stream: &CeremonyId) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM ceremony_snapshots WHERE stream_id = $1")
            .bind(stream.as_str())
            .execute(self.pool.inner())
            .await
            .map_err(|error| sqlx_error(error, "forget ceremony snapshots"))?;
        Ok(())
    }
}
