use async_trait::async_trait;
use made_core::entities::{CeremonyDefinition, PublicationOutcome, PublishedCeremonyDefinition};
use made_core::error::DomainError;
use made_core::ports::CeremonyDefinitionPublicationPort;
use made_core::value_objects::{CeremonyDefinitionDigest, CeremonyName, CeremonyVersion};
use sqlx::Row;

use super::ceremony_store::{decode, encode, sqlx_error};
use super::PostgresCeremonyStore;

fn restore(bytes: &[u8]) -> Result<PublishedCeremonyDefinition, DomainError> {
    let (definition, digest): (CeremonyDefinition, CeremonyDefinitionDigest) =
        decode(bytes, "decode ceremony publication")?;
    let published = PublishedCeremonyDefinition::seal(definition)?;
    if published.digest() != digest {
        return Err(DomainError::InvariantViolated {
            reason: "postgres: publication digest does not match its definition",
        });
    }
    Ok(published)
}

#[async_trait]
impl CeremonyDefinitionPublicationPort for PostgresCeremonyStore {
    async fn publish(
        &self,
        definition: PublishedCeremonyDefinition,
    ) -> Result<PublicationOutcome, DomainError> {
        let payload = encode(
            &(definition.definition(), definition.digest()),
            "encode ceremony publication",
        )?;
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin ceremony publication"))?;
        let inserted = sqlx::query(
            "INSERT INTO ceremony_publications (name, version, digest, payload) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (name, version) DO NOTHING",
        )
        .bind(definition.name().as_str())
        .bind(definition.version().as_str())
        .bind(definition.digest().to_hex())
        .bind(payload)
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "insert ceremony publication"))?;
        if inserted.rows_affected() == 1 {
            transaction
                .commit()
                .await
                .map_err(|error| sqlx_error(error, "commit ceremony publication"))?;
            return Ok(PublicationOutcome::Published(definition));
        }
        let row = sqlx::query(
            "SELECT digest, payload FROM ceremony_publications \
             WHERE name = $1 AND version = $2 FOR UPDATE",
        )
        .bind(definition.name().as_str())
        .bind(definition.version().as_str())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "read ceremony publication occupant"))?;
        let digest: String = row
            .try_get("digest")
            .map_err(|error| sqlx_error(error, "decode ceremony publication digest"))?;
        let payload: Vec<u8> = row
            .try_get("payload")
            .map_err(|error| sqlx_error(error, "decode ceremony publication payload"))?;
        let occupant = restore(&payload)?;
        if occupant.digest().to_hex() != digest {
            return Err(DomainError::InvariantViolated {
                reason: "postgres: publication digest column does not match its payload",
            });
        }
        if occupant.digest() == definition.digest() {
            Ok(PublicationOutcome::AlreadyPublished(occupant))
        } else {
            Ok(PublicationOutcome::VersionOccupied {
                published: occupant.digest(),
                offered: definition.digest(),
            })
        }
    }

    async fn published(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<Option<PublishedCeremonyDefinition>, DomainError> {
        let row = sqlx::query(
            "SELECT payload FROM ceremony_publications WHERE name = $1 AND version = $2",
        )
        .bind(name.as_str())
        .bind(version.as_str())
        .fetch_optional(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "read ceremony publication"))?;
        row.map(|row| {
            let payload: Vec<u8> = row
                .try_get("payload")
                .map_err(|error| sqlx_error(error, "decode ceremony publication payload"))?;
            restore(&payload)
        })
        .transpose()
    }

    async fn catalogue(&self) -> Result<Vec<PublishedCeremonyDefinition>, DomainError> {
        let rows = sqlx::query("SELECT payload FROM ceremony_publications ORDER BY name, version")
            .fetch_all(self.pool.inner())
            .await
            .map_err(|error| sqlx_error(error, "list ceremony publications"))?;
        rows.into_iter()
            .map(|row| {
                let payload: Vec<u8> = row
                    .try_get("payload")
                    .map_err(|error| sqlx_error(error, "decode ceremony publication payload"))?;
                restore(&payload)
            })
            .collect()
    }
}
