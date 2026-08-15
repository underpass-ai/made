use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use made_core::error::DomainError;
use redb::{Database, DatabaseError, ReadOnlyDatabase, ReadableTableMetadata};
use sha2::{Digest, Sha256};

use super::ceremony_store::{encode, JOURNAL, OUTBOX};
use super::error::store_failure;
use super::legacy_instance_migration::LegacyInstanceMigration;
use super::legacy_publication_migration::LegacyPublicationMigration;
use super::legacy_state_migration_receipt::{LegacyStateMigrationReceipt, LEGACY_STATE_MIGRATIONS};

/// Copies a legacy Choreographer store into a new MADE store.
#[derive(Debug)]
pub(super) struct LegacyStateMigrator;

impl LegacyStateMigrator {
    pub(super) fn migrate(
        source_path: &Path,
        destination_path: &Path,
    ) -> Result<LegacyStateMigrationReceipt, DomainError> {
        let source_lock = lock_source_read_only(source_path)?;
        let source_sha256 = clone_source_read_only(source_path, destination_path)?;
        drop(source_lock);

        // The source may need redb recovery after an unclean shutdown. Only
        // the new clone is ever opened writable, so recovery cannot alter the
        // legacy evidence supplied by the operator.
        let destination = Database::open(destination_path)
            .map_err(|error| store_failure(error, "open migration destination"))?;
        let write = destination
            .begin_write()
            .map_err(|error| store_failure(error, "begin migration write"))?;

        let publications = LegacyPublicationMigration::execute(&write)?;
        let instances = LegacyInstanceMigration::execute(&write, &publications)?;

        let audit_records = table_len(&write, JOURNAL, "count legacy audit journal")?;
        let outbox_messages = table_len(&write, OUTBOX, "count legacy outbox")?;
        let receipt = LegacyStateMigrationReceipt::completed(
            source_sha256,
            publications.publication_count(),
            publications.migrated_publications(),
            instances.instance_count(),
            instances.migrated_instances(),
            instances.unresolved_bindings(),
            audit_records,
            outbox_messages,
        );
        {
            let mut migrations = write
                .open_table(LEGACY_STATE_MIGRATIONS)
                .map_err(|error| store_failure(error, "open destination state migrations"))?;
            migrations
                .insert(
                    LegacyStateMigrationReceipt::MIGRATION_ID,
                    encode(&receipt, "encode legacy migration receipt")?.as_slice(),
                )
                .map_err(|error| store_failure(error, "write legacy migration receipt"))?;
        }

        write
            .commit()
            .map_err(|error| store_failure(error, "commit legacy migration"))?;
        Ok(receipt)
    }
}

fn lock_source_read_only(path: &Path) -> Result<Option<ReadOnlyDatabase>, DomainError> {
    match ReadOnlyDatabase::open(path) {
        Ok(database) => Ok(Some(database)),
        Err(DatabaseError::RepairAborted) => {
            tracing::info!(
                migration_id = LegacyStateMigrationReceipt::MIGRATION_ID,
                source_open_mode = "read_only",
                recovery_required = true,
                "legacy state requires recovery in the destination clone"
            );
            Ok(None)
        }
        Err(error) => Err(store_failure(
            error,
            "lock legacy database for read-only migration",
        )),
    }
}

fn clone_source_read_only(
    source_path: &Path,
    destination_path: &Path,
) -> Result<String, DomainError> {
    let mut source = File::open(source_path)
        .map_err(|error| store_failure(error, "open legacy database read-only"))?;
    let mut destination = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(destination_path)
        .map_err(|error| store_failure(error, "create new migration destination"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|error| store_failure(error, "read legacy database"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        destination
            .write_all(&buffer[..read])
            .map_err(|error| store_failure(error, "copy legacy database"))?;
    }
    destination
        .sync_all()
        .map_err(|error| store_failure(error, "sync migration destination"))?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn table_len<K, V>(
    write: &redb::WriteTransaction,
    definition: redb::TableDefinition<'static, K, V>,
    operation: &'static str,
) -> Result<u64, DomainError>
where
    K: redb::Key + 'static,
    V: redb::Value + 'static,
{
    let table = write
        .open_table(definition)
        .map_err(|error| store_failure(error, operation))?;
    table.len().map_err(|error| store_failure(error, operation))
}

#[cfg(test)]
mod tests {
    use made_core::entities::{CeremonyDefinition, CeremonyInstance, PublishedCeremonyDefinition};
    use made_core::ports::{CeremonyDefinitionPublicationPort, CeremonyInstanceRepositoryPort};
    use made_core::value_objects::{
        CeremonyContext, CeremonyDefinitionDigest, CeremonyId, CeremonyName, CeremonyState,
        CeremonyVersion, StateId,
    };
    use redb::ReadableTable;
    use time::OffsetDateTime;

    use super::*;
    use crate::redb::ceremony_store::{
        StoredCeremony, StoredPublication, CEREMONIES, PUBLICATIONS,
    };
    use crate::redb::legacy_definition_binding::LegacyDefinitionBinding;
    use crate::redb::RedbCeremonyStore;

    #[tokio::test]
    async fn imports_an_open_legacy_store_without_mutating_the_source() {
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("choreographer.redb");
        let destination_path = directory.path().join("made.redb");
        let definition = definition();
        let current = PublishedCeremonyDefinition::seal(definition.clone()).unwrap();
        let legacy = LegacyDefinitionBinding::verify(definition, current.digest())
            .unwrap()
            .legacy_digest();

        {
            let store = RedbCeremonyStore::open(&source_path).unwrap();
            store.publish(current.clone()).await.unwrap();
            store
                .save(&CeremonyInstance::start_bound(
                    CeremonyId::new("legacy-instance").unwrap(),
                    &current,
                    CeremonyContext::empty(),
                    OffsetDateTime::UNIX_EPOCH,
                ))
                .await
                .unwrap();
        }
        rewrite_as_legacy(&source_path, legacy);

        // A clone taken while a writer owns the source can require redb
        // recovery. Only the destination is allowed to perform that repair.
        let original_path = directory.path().join("open-choreographer.redb");
        std::fs::rename(&source_path, &original_path).unwrap();
        let legacy_owner = RedbCeremonyStore::open(&original_path).unwrap();
        std::fs::copy(&original_path, &source_path).unwrap();
        drop(legacy_owner);
        let source_before = std::fs::read(&source_path).unwrap();
        let migrated = RedbCeremonyStore::import_legacy(&source_path, &destination_path).unwrap();
        let source_after = std::fs::read(&source_path).unwrap();

        assert_eq!(
            source_after, source_before,
            "the source was opened writable"
        );
        let receipt = migrated.legacy_migration_receipt().unwrap().unwrap();
        assert_eq!(receipt.publications(), 1);
        assert_eq!(receipt.migrated_publications(), 1);
        assert_eq!(receipt.instances(), 1);
        assert_eq!(receipt.migrated_instances(), 1);
        assert_eq!(receipt.unresolved_bindings(), 0);

        let publication = migrated
            .published(current.name(), current.version())
            .await
            .unwrap()
            .unwrap();
        let instance = migrated
            .get(&CeremonyId::new("legacy-instance").unwrap())
            .await
            .unwrap();
        assert_eq!(publication.digest(), current.digest());
        assert_eq!(instance.bound_definition(), Some(current.digest()));

        drop(migrated);
        let reopened = RedbCeremonyStore::open_or_import_legacy(
            directory.path().join("source-no-longer-needed.redb"),
            &destination_path,
        )
        .unwrap();
        assert!(reopened.legacy_migration_receipt().unwrap().is_some());
    }

    fn rewrite_as_legacy(path: &Path, legacy: CeremonyDefinitionDigest) {
        let database = Database::open(path).unwrap();
        let write = database.begin_write().unwrap();
        {
            let mut publications = write.open_table(PUBLICATIONS).unwrap();
            let (key, mut stored) = {
                let entry = publications.iter().unwrap().next().unwrap().unwrap();
                let stored: StoredPublication = serde_json::from_slice(entry.1.value()).unwrap();
                (entry.0.value().to_vec(), stored)
            };
            stored.digest = legacy;
            publications
                .insert(
                    key.as_slice(),
                    serde_json::to_vec(&stored).unwrap().as_slice(),
                )
                .unwrap();
        }
        {
            let mut ceremonies = write.open_table(CEREMONIES).unwrap();
            let (key, stored) = {
                let entry = ceremonies.iter().unwrap().next().unwrap().unwrap();
                let stored: StoredCeremony = serde_json::from_slice(entry.1.value()).unwrap();
                (entry.0.value().to_owned(), stored)
            };
            let mut value = serde_json::to_value(stored).unwrap();
            value["instance"]["bound_definition"] = serde_json::to_value(legacy).unwrap();
            let stored: StoredCeremony = serde_json::from_value(value).unwrap();
            ceremonies
                .insert(
                    key.as_str(),
                    serde_json::to_vec(&stored).unwrap().as_slice(),
                )
                .unwrap();
        }
        write.commit().unwrap();
    }

    fn definition() -> CeremonyDefinition {
        CeremonyDefinition::new(
            CeremonyName::new("legacy_definition").unwrap(),
            CeremonyVersion::v1(),
            None,
            [],
            [],
            [CeremonyState::initial(StateId::new("OPEN").unwrap())],
            [],
            [],
            [],
            [],
        )
        .unwrap()
    }
}
