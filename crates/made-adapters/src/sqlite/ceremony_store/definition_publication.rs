use async_trait::async_trait;
use made_core::entities::{
    CeremonyCatalogueEntry, PublicationOutcome, PublishedCeremonyDefinition,
    UnreadableCeremonyPublication,
};
use made_core::error::DomainError;
use made_core::ports::CeremonyDefinitionPublicationPort;
use made_core::value_objects::{CeremonyName, CeremonyVersion};

use crate::engine::{Key, Table};
use crate::sqlite::keys::{published, published_identity};
use crate::sqlite::StoredPublication;

use super::{decode, encode, SqliteCeremonyStore};

#[async_trait]
impl CeremonyDefinitionPublicationPort for SqliteCeremonyStore {
    /// The occupant is read and the slot written inside one write
    /// transaction, so two callers cannot publish different content
    /// under one version.
    async fn publish(
        &self,
        definition: PublishedCeremonyDefinition,
    ) -> Result<PublicationOutcome, DomainError> {
        self.blocking("publish", move |engine| {
            let key = published(definition.name(), definition.version());
            let mut tx = engine.begin_write()?;
            let outcome = {
                let occupant: Option<StoredPublication> = tx
                    .get(Table::Publications, Key::Bytes(&key))?
                    .map(|value| decode(&value, "decode publication"))
                    .transpose()?;

                match occupant {
                    Some(occupant) if occupant.digest == definition.digest() => {
                        PublicationOutcome::AlreadyPublished(occupant.restore()?)
                    }
                    Some(occupant) => PublicationOutcome::VersionOccupied {
                        published: occupant.digest,
                        offered: definition.digest(),
                    },
                    None => {
                        tx.insert(
                            Table::Publications,
                            Key::Bytes(&key),
                            &encode(&StoredPublication::seal(&definition), "encode publication")?,
                        )?;
                        PublicationOutcome::Published(definition)
                    }
                }
            };

            if outcome.is_conflict() {
                return Ok(outcome);
            }
            tx.commit()?;
            Ok(outcome)
        })
        .await
    }

    async fn published(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<Option<PublishedCeremonyDefinition>, DomainError> {
        let key = published(name, version);
        self.blocking("published", move |engine| {
            let tx = engine.begin_read()?;
            let stored: Option<StoredPublication> = tx
                .get(Table::Publications, Key::Bytes(&key))?
                .map(|value| decode(&value, "decode publication"))
                .transpose()?;
            stored.map(StoredPublication::restore).transpose()
        })
        .await
    }

    async fn catalogue(&self) -> Result<Vec<PublishedCeremonyDefinition>, DomainError> {
        self.blocking("catalogue", move |engine| {
            let tx = engine.begin_read()?;
            tx.scan_bytes(Table::Publications)?
                .into_iter()
                .map(|(_, value)| {
                    decode::<StoredPublication>(&value, "decode publication")?.restore()
                })
                .collect()
        })
        .await
    }

    /// Every row, and for a row that cannot be restored the reason,
    /// named from its payload when that decodes and from its key when
    /// it does not.
    async fn catalogue_entries(&self) -> Result<Vec<CeremonyCatalogueEntry>, DomainError> {
        self.blocking("catalogue entries", move |engine| {
            let tx = engine.begin_read()?;
            tx.scan_bytes(Table::Publications)?
                .into_iter()
                .map(|(key, value)| catalogue_entry(&key, &value))
                .collect()
        })
        .await
    }
}

fn catalogue_entry(key: &[u8], value: &[u8]) -> Result<CeremonyCatalogueEntry, DomainError> {
    let stored = match decode::<StoredPublication>(value, "decode publication") {
        Ok(stored) => stored,
        Err(defect) => {
            let (name, version) = published_identity(key)?;
            return Ok(CeremonyCatalogueEntry::Unreadable(
                UnreadableCeremonyPublication::new(name, version, None, defect),
            ));
        }
    };
    let name = stored.definition.name().clone();
    let version = stored.definition.version().clone();
    let recorded = stored.digest;
    Ok(match stored.restore() {
        Ok(published) => CeremonyCatalogueEntry::Readable(Box::new(published)),
        Err(defect) => CeremonyCatalogueEntry::Unreadable(UnreadableCeremonyPublication::new(
            name,
            version,
            Some(recorded),
            defect,
        )),
    })
}
