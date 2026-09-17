//! `made-mcp migrate-store <path>` — the one way back into a store
//! written before ceremonies were streams (ADR-012).
//!
//! Since the engine became event-sourced, a session from v0.3.x is in
//! the file and invisible: it has a snapshot and a journal of
//! payload-less receipts, and the engine reads streams. The import
//! opens a stream for each one with a single genesis event carrying
//! the snapshot, and proves the fold before anything replaces the
//! operator's file.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::usecases::ImportPreStreamInstancesUseCase;
use made_core::value_objects::{AuditActor, AuditActorKind, CeremonyId};
use time::OffsetDateTime;

mod working_copy;

use working_copy::WorkingCopy;

/// Who the genesis records name as their author.
///
/// The command rather than the operator: an import is not something
/// anyone at the table did, and recording a person as the author of a
/// session's first event would put a name on a fact they had no part
/// in.
const MIGRATION_ACTOR: &str = "made-mcp migrate-store";

/// What one run of the command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrateStoreOutcome {
    /// Every session in the store already had a stream. Nothing was
    /// copied, nothing was installed, nothing was moved.
    AlreadyMigrated,
    /// The sessions listed were imported and the migrated store was
    /// installed; the original is at `backup`.
    Migrated {
        imported: Vec<CeremonyId>,
        backup: PathBuf,
    },
}

/// Migrate the store at `path`, or say why it was not migrated.
///
/// Running it twice is safe by construction: the second run finds no
/// session without a stream, discards its copy and reports that the
/// store was already migrated. The original file is never opened for
/// writing on either run.
pub async fn migrate_store(path: &Path) -> Result<MigrateStoreOutcome, String> {
    let copy = WorkingCopy::create(path)?;
    match import(copy.path()).await {
        Ok(imported) if imported.is_empty() => {
            copy.discard()?;
            Ok(MigrateStoreOutcome::AlreadyMigrated)
        }
        Ok(imported) => {
            let backup = copy.install()?;
            Ok(MigrateStoreOutcome::Migrated { imported, backup })
        }
        Err(error) => {
            copy.discard()?;
            Err(error)
        }
    }
}

/// Import every pre-stream session of the store at `working`.
///
/// The store handle is dropped before this returns, so SQLite has
/// checkpointed and closed the file by the time the caller renames it.
async fn import(working: &Path) -> Result<Vec<CeremonyId>, String> {
    let actor = AuditActor::new(MIGRATION_ACTOR, AuditActorKind::Engine, None)
        .map_err(|error| format!("the migration cannot name its author: {error}"))?;
    let store = Arc::new(
        SqliteCeremonyStore::open(working)
            .map_err(|error| format!("cannot open the copied store: {error}"))?,
    );
    let report = ImportPreStreamInstancesUseCase::new(store.clone(), store)
        .execute(actor, OffsetDateTime::now_utc())
        .await
        .map_err(|error| format!("the import did not hold: {error}"))?;
    Ok(report.imported().to_vec())
}

/// The lines the command prints, in the order it prints them.
///
/// Rendered here rather than in `main` so that what an operator is told
/// is testable without a process: a migration that installed a file
/// and said nothing about where the original went would be worse than
/// one that failed.
#[must_use]
pub fn migrate_store_report(path: &Path, outcome: &MigrateStoreOutcome) -> Vec<String> {
    let mut lines = vec![format!("made-mcp migrate-store: {}", path.display())];
    match outcome {
        MigrateStoreOutcome::AlreadyMigrated => lines
            .push("  every session already has an event stream; nothing was changed".to_owned()),
        MigrateStoreOutcome::Migrated { imported, backup } => {
            lines.push(format!("  sessions imported: {}", imported.len()));
            for ceremony_id in imported {
                lines.push(format!("    {}", ceremony_id.as_str()));
            }
            lines.push(
                "  verified: every imported stream folds back to the session it imported"
                    .to_owned(),
            );
            lines.push(format!("  original kept at: {}", backup.display()));
            lines.push(format!("  migrated store installed at: {}", path.display()));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_already_migrated_store_is_told_that_nothing_changed() {
        let lines = migrate_store_report(
            Path::new("/srv/ceremonies.sqlite3"),
            &MigrateStoreOutcome::AlreadyMigrated,
        );

        assert_eq!(lines[0], "made-mcp migrate-store: /srv/ceremonies.sqlite3");
        assert!(lines[1].contains("nothing was changed"));
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn a_migration_names_every_session_and_where_the_original_went() {
        let lines = migrate_store_report(
            Path::new("/srv/ceremonies.sqlite3"),
            &MigrateStoreOutcome::Migrated {
                imported: vec![
                    CeremonyId::new("session-1").unwrap(),
                    CeremonyId::new("session-2").unwrap(),
                ],
                backup: PathBuf::from("/srv/ceremonies.sqlite3.pre-stream.backup"),
            },
        );

        let report = lines.join("\n");
        assert!(report.contains("sessions imported: 2"));
        assert!(report.contains("session-1"));
        assert!(report.contains("session-2"));
        assert!(report.contains("folds back"));
        assert!(report.contains("/srv/ceremonies.sqlite3.pre-stream.backup"));
    }
}
