use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use redb::ReadableTable;

use crate::engine::redb::PUBLICATIONS;

use super::ceremony_store::{decode, encode};
use super::error::store_failure;
use super::legacy_definition_binding::LegacyDefinitionBinding;
use super::StoredPublication;

/// Verified publication mappings imported into the MADE digest scheme.
#[derive(Debug)]
pub(super) struct LegacyPublicationMigration {
    bindings: Vec<LegacyDefinitionBinding>,
    publication_count: u64,
    migrated_publications: u64,
}

impl LegacyPublicationMigration {
    pub(super) fn execute(write: &redb::WriteTransaction) -> Result<Self, DomainError> {
        let mut publications = write
            .open_table(PUBLICATIONS)
            .map_err(|error| store_failure(error, "open destination publications"))?;
        let mut entries = Vec::new();
        for entry in publications
            .range::<&[u8]>(..)
            .map_err(|error| store_failure(error, "scan legacy publications"))?
        {
            let (key, value) =
                entry.map_err(|error| store_failure(error, "read legacy publication"))?;
            let stored: StoredPublication = decode(value.value(), "decode legacy publication")?;
            entries.push((key.value().to_vec(), stored));
        }

        let publication_count = entries.len() as u64;
        let mut migrated_publications = 0_u64;
        let mut bindings = Vec::with_capacity(entries.len());
        for (key, stored) in entries {
            let binding = LegacyDefinitionBinding::verify(stored.definition, stored.digest)?;
            if binding.publication_requires_migration() {
                migrated_publications = migrated_publications.saturating_add(1);
            }
            publications
                .insert(
                    key.as_slice(),
                    encode(
                        &StoredPublication::seal(binding.published()),
                        "encode migrated publication",
                    )?
                    .as_slice(),
                )
                .map_err(|error| store_failure(error, "write migrated publication"))?;
            bindings.push(binding);
        }

        Ok(Self {
            bindings,
            publication_count,
            migrated_publications,
        })
    }

    pub(super) fn binding_for(
        &self,
        instance: &CeremonyInstance,
    ) -> Option<&LegacyDefinitionBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.matches(instance))
    }

    pub(super) fn publication_count(&self) -> u64 {
        self.publication_count
    }

    pub(super) fn migrated_publications(&self) -> u64 {
        self.migrated_publications
    }
}
