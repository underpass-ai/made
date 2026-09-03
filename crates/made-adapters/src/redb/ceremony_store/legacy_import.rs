use std::path::Path;

use made_core::error::DomainError;

use crate::engine::{Key, Table};

use super::path_identity::same_path;
use super::{decode, RedbCeremonyStore};
use crate::redb::legacy_state_migration_receipt::LegacyStateMigrationReceipt;
use crate::redb::legacy_state_migrator::LegacyStateMigrator;

impl RedbCeremonyStore {
    /// Import a pre-rename Choreographer redb file into a new MADE store.
    ///
    /// The source is opened with read-only file permissions. The destination
    /// must not exist, so migration cannot overwrite either the legacy
    /// evidence or an independently created MADE store.
    pub fn import_legacy(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<Self, DomainError> {
        let source = source.as_ref();
        let destination = destination.as_ref();

        if same_path(source, destination) {
            tracing::error!(
                migration_id = LegacyStateMigrationReceipt::MIGRATION_ID,
                "legacy state migration source and destination are the same file"
            );
            return Err(DomainError::InvariantViolated {
                reason: "legacy state migration requires different source and destination files",
            });
        }
        if destination.exists() {
            tracing::error!(
                migration_id = LegacyStateMigrationReceipt::MIGRATION_ID,
                "legacy state migration destination already exists"
            );
            return Err(DomainError::Conflict {
                what: "legacy_state_migration_destination",
            });
        }

        let receipt = LegacyStateMigrator::migrate(source, destination)?;
        tracing::info!(
            migration_id = LegacyStateMigrationReceipt::MIGRATION_ID,
            source_open_mode = "read_only",
            publications = receipt.publications(),
            migrated_publications = receipt.migrated_publications(),
            instances = receipt.instances(),
            migrated_instances = receipt.migrated_instances(),
            unresolved_bindings = receipt.unresolved_bindings(),
            audit_records = receipt.audit_records(),
            outbox_messages = receipt.outbox_messages(),
            source_sha256 = receipt.source_sha256(),
            "made legacy state migration completed"
        );
        Self::open(destination)
    }

    /// Import once, then reopen the already migrated destination on later
    /// process starts.
    pub fn open_or_import_legacy(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<Self, DomainError> {
        let source = source.as_ref();
        let destination = destination.as_ref();
        if same_path(source, destination) {
            return Err(DomainError::InvariantViolated {
                reason: "legacy state migration requires different source and destination files",
            });
        }
        if !destination.exists() {
            return Self::import_legacy(source, destination);
        }

        let store = Self::open(destination)?;
        let receipt = store
            .legacy_migration_receipt()?
            .ok_or(DomainError::Conflict {
                what: "legacy_state_migration_destination_without_receipt",
            })?;
        tracing::info!(
            migration_id = LegacyStateMigrationReceipt::MIGRATION_ID,
            outcome = "already_completed",
            source_open_mode = "not_reopened",
            publications = receipt.publications(),
            migrated_publications = receipt.migrated_publications(),
            instances = receipt.instances(),
            migrated_instances = receipt.migrated_instances(),
            unresolved_bindings = receipt.unresolved_bindings(),
            source_sha256 = receipt.source_sha256(),
            "made legacy state migration receipt verified"
        );
        Ok(store)
    }

    /// The durable receipt written by a completed legacy import, if this
    /// store was created through [`Self::import_legacy`].
    pub fn legacy_migration_receipt(
        &self,
    ) -> Result<Option<LegacyStateMigrationReceipt>, DomainError> {
        let tx = self.engine.begin_read()?;
        tx.get(
            Table::LegacyStateMigrations,
            Key::Str(LegacyStateMigrationReceipt::MIGRATION_ID),
        )?
        .map(|value| decode(&value, "decode legacy migration receipt"))
        .transpose()
    }
}
