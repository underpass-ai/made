use std::path::Path;

use made_core::error::DomainError;

use crate::engine::detect::{engine_of, StorageEngine};
use crate::engine::{Key, Table};
use crate::redb::ConversionReceipt;

use super::path_identity::same_path;
use super::RedbCeremonyStore;

impl RedbCeremonyStore {
    /// Copy a store to `destination` on `engine`, table for table.
    ///
    /// This is how a store changes engines. It is a copy rather than a replay
    /// of history, and that is not a shortcut: a ceremony store is state plus
    /// an audit journal, not a log with derived projections. Replaying the
    /// journal would rebuild the facts and lose the very thing the journal is
    /// evidence *of* — the instances, the outbox and the publications are
    /// authoritative, not derivable.
    ///
    /// What makes the copy safe is that both engines answer the same seam, so
    /// "every row of every table, in key order" means the same thing on each
    /// side. Rows move as opaque bytes: nothing is decoded, so nothing can be
    /// re-encoded differently.
    ///
    /// The source is only read. The destination must not already hold a
    /// store — converting into occupied memory would be a worse failure than
    /// the one it fixes.
    pub fn convert(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
        engine: StorageEngine,
    ) -> Result<ConversionReceipt, DomainError> {
        let source_path = source.as_ref();
        let destination_path = destination.as_ref();

        if same_path(source_path, destination_path) {
            tracing::error!("ceremony store conversion source and destination are the same file");
            return Err(DomainError::InvariantViolated {
                reason: "store conversion requires different source and destination files",
            });
        }
        let Some(source_engine) = engine_of(source_path)? else {
            tracing::error!(path = %source_path.display(), "no ceremony store to convert");
            return Err(DomainError::NotFound {
                what: "ceremony_store",
            });
        };
        if source_engine == engine {
            tracing::error!(
                engine = engine.name(),
                "the ceremony store already runs on the requested engine"
            );
            return Err(DomainError::Conflict {
                what: "the store already runs on that engine",
            });
        }
        if engine_of(destination_path)?.is_some() {
            tracing::error!(
                path = %destination_path.display(),
                "refusing to convert into a path that already holds a ceremony store"
            );
            return Err(DomainError::AlreadyExists {
                what: "ceremony_store",
            });
        }

        let from = Self::on(source_path, source_engine)?;
        let into = Self::on(destination_path, engine)?;

        let read = from.engine.begin_read()?;
        let mut write = into.engine.begin_write()?;
        let mut receipt = ConversionReceipt {
            source_engine,
            destination_engine: engine,
            ceremonies: 0,
            journal_records: 0,
            outbox_messages: 0,
            publications: 0,
        };

        for (table, counter) in [
            (Table::Ceremonies, &mut receipt.ceremonies),
            (Table::LegacyStateMigrations, &mut 0),
        ] {
            for (key, value) in read.scan_str(table)? {
                write.insert(table, Key::Str(&key), &value)?;
                *counter += 1;
            }
        }
        for (table, counter) in [
            (Table::Journal, &mut receipt.journal_records),
            (Table::Outbox, &mut receipt.outbox_messages),
            (Table::Publications, &mut receipt.publications),
        ] {
            for (key, value) in read.scan_bytes(table)? {
                write.insert(table, Key::Bytes(&key), &value)?;
                *counter += 1;
            }
        }

        write.commit()?;
        Ok(receipt)
    }
}
