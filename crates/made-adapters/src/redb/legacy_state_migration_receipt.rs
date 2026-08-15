use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use redb::TableDefinition;

pub(super) const LEGACY_STATE_MIGRATIONS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("state_migrations");

/// Durable evidence that a read-only legacy database was imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyStateMigrationReceipt {
    migration_id: String,
    source_sha256: String,
    source_open_mode: String,
    legacy_digest_scheme: String,
    current_digest_scheme: String,
    publications: u64,
    migrated_publications: u64,
    instances: u64,
    migrated_instances: u64,
    unresolved_bindings: u64,
    audit_records: u64,
    outbox_messages: u64,
    #[serde(with = "time::serde::rfc3339")]
    completed_at: OffsetDateTime,
}

impl LegacyStateMigrationReceipt {
    pub const MIGRATION_ID: &'static str = "choreographer-v1-to-made-v1";

    pub(super) fn completed(
        source_sha256: String,
        publications: u64,
        migrated_publications: u64,
        instances: u64,
        migrated_instances: u64,
        unresolved_bindings: u64,
        audit_records: u64,
        outbox_messages: u64,
    ) -> Self {
        Self {
            migration_id: Self::MIGRATION_ID.to_owned(),
            source_sha256,
            source_open_mode: "read_only".to_owned(),
            legacy_digest_scheme: "underpass.choreo.ceremony-definition.v1".to_owned(),
            current_digest_scheme: "underpass.made.ceremony-definition.v1".to_owned(),
            publications,
            migrated_publications,
            instances,
            migrated_instances,
            unresolved_bindings,
            audit_records,
            outbox_messages,
            completed_at: OffsetDateTime::now_utc(),
        }
    }

    #[must_use]
    pub fn migration_id(&self) -> &str {
        &self.migration_id
    }

    #[must_use]
    pub fn source_open_mode(&self) -> &str {
        &self.source_open_mode
    }

    #[must_use]
    pub fn legacy_digest_scheme(&self) -> &str {
        &self.legacy_digest_scheme
    }

    #[must_use]
    pub fn current_digest_scheme(&self) -> &str {
        &self.current_digest_scheme
    }

    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    #[must_use]
    pub fn publications(&self) -> u64 {
        self.publications
    }

    #[must_use]
    pub fn migrated_publications(&self) -> u64 {
        self.migrated_publications
    }

    #[must_use]
    pub fn instances(&self) -> u64 {
        self.instances
    }

    #[must_use]
    pub fn migrated_instances(&self) -> u64 {
        self.migrated_instances
    }

    #[must_use]
    pub fn unresolved_bindings(&self) -> u64 {
        self.unresolved_bindings
    }

    #[must_use]
    pub fn audit_records(&self) -> u64 {
        self.audit_records
    }

    #[must_use]
    pub fn outbox_messages(&self) -> u64 {
        self.outbox_messages
    }

    #[must_use]
    pub fn completed_at(&self) -> OffsetDateTime {
        self.completed_at
    }
}
