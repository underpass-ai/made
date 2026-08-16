use made_core::error::DomainError;
use redb::ReadableTable;

use crate::engine::redb::CEREMONIES;

use super::ceremony_store::{decode, encode, StoredCeremony};
use super::error::store_failure;
use super::legacy_publication_migration::LegacyPublicationMigration;

/// Counts and applies instance binding changes for one legacy import.
#[derive(Debug)]
pub(super) struct LegacyInstanceMigration {
    instance_count: u64,
    migrated_instances: u64,
    unresolved_bindings: u64,
}

impl LegacyInstanceMigration {
    pub(super) fn execute(
        write: &redb::WriteTransaction,
        publications: &LegacyPublicationMigration,
    ) -> Result<Self, DomainError> {
        let mut ceremonies = write
            .open_table(CEREMONIES)
            .map_err(|error| store_failure(error, "open destination ceremonies"))?;
        let mut entries = Vec::new();
        for entry in ceremonies
            .range::<&str>(..)
            .map_err(|error| store_failure(error, "scan legacy ceremonies"))?
        {
            let (key, value) =
                entry.map_err(|error| store_failure(error, "read legacy ceremony"))?;
            let stored: StoredCeremony = decode(value.value(), "decode legacy ceremony")?;
            entries.push((key.value().to_owned(), stored));
        }

        let instance_count = entries.len() as u64;
        let mut migrated_instances = 0_u64;
        let mut unresolved_bindings = 0_u64;
        for (key, mut stored) in entries {
            if stored.instance.bound_definition().is_some() {
                match publications.binding_for(&stored.instance) {
                    Some(binding) if binding.migrate_instance(&mut stored.instance)? => {
                        stored.revision = stored.revision.next();
                        migrated_instances = migrated_instances.saturating_add(1);
                    }
                    Some(_) => {}
                    None => unresolved_bindings = unresolved_bindings.saturating_add(1),
                }
            }
            ceremonies
                .insert(
                    key.as_str(),
                    encode(&stored, "encode migrated ceremony")?.as_slice(),
                )
                .map_err(|error| store_failure(error, "write migrated ceremony"))?;
        }

        Ok(Self {
            instance_count,
            migrated_instances,
            unresolved_bindings,
        })
    }

    pub(super) fn instance_count(&self) -> u64 {
        self.instance_count
    }

    pub(super) fn migrated_instances(&self) -> u64 {
        self.migrated_instances
    }

    pub(super) fn unresolved_bindings(&self) -> u64 {
        self.unresolved_bindings
    }
}
