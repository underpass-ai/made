//! The redb engine behind the seam.
//!
//! On-disk layout is unchanged from before the seam existed: the same table
//! names, the same key and value types, the same file. redb records the key
//! and value type names in the table metadata and refuses a definition that
//! disagrees, so an existing store keeps opening.
//!
//! Tables are opened per operation and dropped before the next. A redb write
//! transaction refuses to open a table that already has a live handle, and
//! holding handles across seam calls would need the transaction and its
//! borrowers in one self-referential struct. Opening an existing table is a
//! root lookup inside the transaction.

use std::path::Path;
use std::sync::Arc;

use made_core::error::DomainError;
use redb::{
    Database, ReadTransaction, ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction,
};

use super::{
    key_shape_mismatch, scan_shape_mismatch, BytesRow, Engine, Key, KeyShape, ReadTx, StrRow,
    Table, WriteTx,
};
use crate::redb::error::store_failure;

// Names and types are load-bearing: they are what an existing store on disk
// was written with.
//
// Visible to the legacy migrators, which deliberately do not go through the
// seam: they open a foreign pre-rename file and a destination database
// directly, so they need redb's own definitions rather than an engine.
pub(crate) const CEREMONIES: TableDefinition<&str, &[u8]> =
    TableDefinition::new("ceremony_instances");
pub(crate) const JOURNAL: TableDefinition<&[u8], &[u8]> = TableDefinition::new("audit_journal");
pub(crate) const OUTBOX: TableDefinition<&[u8], &[u8]> = TableDefinition::new("outbox");
pub(crate) const PUBLICATIONS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("published_definitions");
pub(crate) const LEGACY_STATE_MIGRATIONS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("state_migrations");

/// One open redb file.
#[derive(Debug, Clone)]
pub(crate) struct RedbEngine {
    database: Arc<Database>,
}

impl RedbEngine {
    /// Opens (or creates) `path` and materializes every table, so read
    /// transactions never race table existence.
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let database =
            Database::create(path).map_err(|error| store_failure(error, "open database"))?;
        let engine = Self {
            database: Arc::new(database),
        };
        engine.create_tables()?;
        Ok(engine)
    }

    fn create_tables(&self) -> Result<(), DomainError> {
        let write = self
            .database
            .begin_write()
            .map_err(|error| store_failure(error, "open tables"))?;
        {
            write
                .open_table(CEREMONIES)
                .map_err(|error| store_failure(error, "open ceremonies table"))?;
            write
                .open_table(JOURNAL)
                .map_err(|error| store_failure(error, "open journal table"))?;
            write
                .open_table(OUTBOX)
                .map_err(|error| store_failure(error, "open outbox table"))?;
            write
                .open_table(PUBLICATIONS)
                .map_err(|error| store_failure(error, "open publications table"))?;
            write
                .open_table(LEGACY_STATE_MIGRATIONS)
                .map_err(|error| store_failure(error, "open state migrations table"))?;
        }
        write
            .commit()
            .map_err(|error| store_failure(error, "create tables"))
    }
}

impl Engine for RedbEngine {
    fn begin_read(&self) -> Result<Box<dyn ReadTx + '_>, DomainError> {
        let tx = self
            .database
            .begin_read()
            .map_err(|error| store_failure(error, "begin read"))?;
        Ok(Box::new(RedbRead { tx }))
    }

    fn begin_write(&self) -> Result<Box<dyn WriteTx + '_>, DomainError> {
        let tx = self
            .database
            .begin_write()
            .map_err(|error| store_failure(error, "begin write"))?;
        Ok(Box::new(RedbWrite { tx }))
    }
}

// ---------------------------------------------------------------- helpers --

fn get_row<K: redb::Key + 'static>(
    table: &impl ReadableTable<K, &'static [u8]>,
    key: K::SelfType<'_>,
    op: &'static str,
) -> Result<Option<Vec<u8>>, DomainError> {
    Ok(table
        .get(key)
        .map_err(|error| store_failure(error, op))?
        .map(|guard| guard.value().to_vec()))
}

fn scan_str_rows(
    table: &impl ReadableTable<&'static str, &'static [u8]>,
    op: &'static str,
) -> Result<Vec<StrRow>, DomainError> {
    let mut rows = Vec::new();
    for row in table.iter().map_err(|error| store_failure(error, op))? {
        let (key, value) = row.map_err(|error| store_failure(error, op))?;
        rows.push((key.value().to_string(), value.value().to_vec()));
    }
    Ok(rows)
}

fn scan_bytes_rows(
    table: &impl ReadableTable<&'static [u8], &'static [u8]>,
    op: &'static str,
) -> Result<Vec<BytesRow>, DomainError> {
    let mut rows = Vec::new();
    for row in table.iter().map_err(|error| store_failure(error, op))? {
        let (key, value) = row.map_err(|error| store_failure(error, op))?;
        rows.push((key.value().to_vec(), value.value().to_vec()));
    }
    Ok(rows)
}

fn scan_bytes_in_range(
    table: &impl ReadableTable<&'static [u8], &'static [u8]>,
    start: &[u8],
    end: &[u8],
    op: &'static str,
) -> Result<Vec<BytesRow>, DomainError> {
    let mut rows = Vec::new();
    // Inclusive on both ends: see the seam's contract for why.
    for row in table
        .range(start..=end)
        .map_err(|error| store_failure(error, op))?
    {
        let (key, value) = row.map_err(|error| store_failure(error, op))?;
        rows.push((key.value().to_vec(), value.value().to_vec()));
    }
    Ok(rows)
}

// --------------------------------------------------------------- read txn --

struct RedbRead {
    tx: ReadTransaction,
}

macro_rules! read_table {
    ($tx:expr, $def:expr, $empty:expr, $op:expr, |$t:ident| $body:expr) => {{
        match $tx.open_table($def) {
            Ok($t) => $body,
            Err(::redb::TableError::TableDoesNotExist(_)) => Ok($empty),
            Err(error) => Err(store_failure(error, $op)),
        }
    }};
}

impl ReadTx for RedbRead {
    fn get(&self, table: Table, key: Key<'_>) -> Result<Option<Vec<u8>>, DomainError> {
        let tx = &self.tx;
        match (table, key) {
            (Table::Ceremonies, Key::Str(k)) => {
                read_table!(tx, CEREMONIES, None, "read ceremony", |t| get_row(
                    &t,
                    k,
                    "read ceremony"
                ))
            }
            (Table::LegacyStateMigrations, Key::Str(k)) => {
                read_table!(tx, LEGACY_STATE_MIGRATIONS, None, "read migration", |t| {
                    get_row(&t, k, "read migration")
                })
            }
            (Table::Journal, Key::Bytes(k)) => {
                read_table!(tx, JOURNAL, None, "read journal", |t| get_row(
                    &t,
                    k,
                    "read journal"
                ))
            }
            (Table::Outbox, Key::Bytes(k)) => {
                read_table!(tx, OUTBOX, None, "read outbox", |t| get_row(
                    &t,
                    k,
                    "read outbox"
                ))
            }
            (Table::Publications, Key::Bytes(k)) => {
                read_table!(tx, PUBLICATIONS, None, "read publication", |t| get_row(
                    &t,
                    k,
                    "read publication"
                ))
            }
            (table, key) => Err(key_shape_mismatch(table, key.shape())),
        }
    }

    fn scan_str(&self, table: Table) -> Result<Vec<StrRow>, DomainError> {
        let tx = &self.tx;
        match table {
            Table::Ceremonies => {
                read_table!(tx, CEREMONIES, Vec::new(), "scan ceremonies", |t| {
                    scan_str_rows(&t, "scan ceremonies")
                })
            }
            Table::LegacyStateMigrations => {
                read_table!(
                    tx,
                    LEGACY_STATE_MIGRATIONS,
                    Vec::new(),
                    "scan migrations",
                    |t| { scan_str_rows(&t, "scan migrations") }
                )
            }
            other => Err(scan_shape_mismatch(other, KeyShape::Str)),
        }
    }

    fn scan_bytes(&self, table: Table) -> Result<Vec<BytesRow>, DomainError> {
        let tx = &self.tx;
        match table {
            Table::Journal => read_table!(tx, JOURNAL, Vec::new(), "scan journal", |t| {
                scan_bytes_rows(&t, "scan journal")
            }),
            Table::Outbox => read_table!(tx, OUTBOX, Vec::new(), "scan outbox", |t| {
                scan_bytes_rows(&t, "scan outbox")
            }),
            Table::Publications => {
                read_table!(tx, PUBLICATIONS, Vec::new(), "scan publications", |t| {
                    scan_bytes_rows(&t, "scan publications")
                })
            }
            other => Err(scan_shape_mismatch(other, KeyShape::Bytes)),
        }
    }

    fn scan_bytes_range(
        &self,
        table: Table,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<BytesRow>, DomainError> {
        let tx = &self.tx;
        match table {
            Table::Journal => read_table!(tx, JOURNAL, Vec::new(), "scan journal scope", |t| {
                scan_bytes_in_range(&t, start, end, "scan journal scope")
            }),
            Table::Outbox => read_table!(tx, OUTBOX, Vec::new(), "scan outbox scope", |t| {
                scan_bytes_in_range(&t, start, end, "scan outbox scope")
            }),
            Table::Publications => {
                read_table!(
                    tx,
                    PUBLICATIONS,
                    Vec::new(),
                    "scan publication scope",
                    |t| { scan_bytes_in_range(&t, start, end, "scan publication scope") }
                )
            }
            other => Err(scan_shape_mismatch(other, KeyShape::Bytes)),
        }
    }
}

// -------------------------------------------------------------- write txn --

struct RedbWrite {
    tx: WriteTransaction,
}

/// Opens `def` read-write inside the write transaction, creating it if
/// absent, and applies `body`.
macro_rules! write_table {
    ($tx:expr, $def:expr, $op:expr, |$t:ident| $body:expr) => {{
        let mut $t = $tx
            .open_table($def)
            .map_err(|error| store_failure(error, $op))?;
        $body
    }};
}

/// The same open, for a read inside a write transaction: this transaction's
/// own writes are visible and the handle is not mutated.
macro_rules! peek_table {
    ($tx:expr, $def:expr, $op:expr, |$t:ident| $body:expr) => {{
        let $t = $tx
            .open_table($def)
            .map_err(|error| store_failure(error, $op))?;
        $body
    }};
}

impl ReadTx for RedbWrite {
    fn get(&self, table: Table, key: Key<'_>) -> Result<Option<Vec<u8>>, DomainError> {
        let tx = &self.tx;
        match (table, key) {
            (Table::Ceremonies, Key::Str(k)) => {
                peek_table!(tx, CEREMONIES, "read ceremony", |t| get_row(
                    &t,
                    k,
                    "read ceremony"
                ))
            }
            (Table::LegacyStateMigrations, Key::Str(k)) => {
                peek_table!(tx, LEGACY_STATE_MIGRATIONS, "read migration", |t| get_row(
                    &t,
                    k,
                    "read migration"
                ))
            }
            (Table::Journal, Key::Bytes(k)) => {
                peek_table!(tx, JOURNAL, "read journal", |t| get_row(
                    &t,
                    k,
                    "read journal"
                ))
            }
            (Table::Outbox, Key::Bytes(k)) => {
                peek_table!(tx, OUTBOX, "read outbox", |t| get_row(&t, k, "read outbox"))
            }
            (Table::Publications, Key::Bytes(k)) => {
                peek_table!(tx, PUBLICATIONS, "read publication", |t| get_row(
                    &t,
                    k,
                    "read publication"
                ))
            }
            (table, key) => Err(key_shape_mismatch(table, key.shape())),
        }
    }

    fn scan_str(&self, table: Table) -> Result<Vec<StrRow>, DomainError> {
        let tx = &self.tx;
        match table {
            Table::Ceremonies => peek_table!(tx, CEREMONIES, "scan ceremonies", |t| {
                scan_str_rows(&t, "scan ceremonies")
            }),
            Table::LegacyStateMigrations => {
                peek_table!(tx, LEGACY_STATE_MIGRATIONS, "scan migrations", |t| {
                    scan_str_rows(&t, "scan migrations")
                })
            }
            other => Err(scan_shape_mismatch(other, KeyShape::Str)),
        }
    }

    fn scan_bytes(&self, table: Table) -> Result<Vec<BytesRow>, DomainError> {
        let tx = &self.tx;
        match table {
            Table::Journal => peek_table!(tx, JOURNAL, "scan journal", |t| scan_bytes_rows(
                &t,
                "scan journal"
            )),
            Table::Outbox => peek_table!(tx, OUTBOX, "scan outbox", |t| scan_bytes_rows(
                &t,
                "scan outbox"
            )),
            Table::Publications => peek_table!(tx, PUBLICATIONS, "scan publications", |t| {
                scan_bytes_rows(&t, "scan publications")
            }),
            other => Err(scan_shape_mismatch(other, KeyShape::Bytes)),
        }
    }

    fn scan_bytes_range(
        &self,
        table: Table,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<BytesRow>, DomainError> {
        let tx = &self.tx;
        match table {
            Table::Journal => peek_table!(tx, JOURNAL, "scan journal scope", |t| {
                scan_bytes_in_range(&t, start, end, "scan journal scope")
            }),
            Table::Outbox => peek_table!(tx, OUTBOX, "scan outbox scope", |t| {
                scan_bytes_in_range(&t, start, end, "scan outbox scope")
            }),
            Table::Publications => peek_table!(tx, PUBLICATIONS, "scan publication scope", |t| {
                scan_bytes_in_range(&t, start, end, "scan publication scope")
            }),
            other => Err(scan_shape_mismatch(other, KeyShape::Bytes)),
        }
    }
}

impl WriteTx for RedbWrite {
    fn insert(&mut self, table: Table, key: Key<'_>, value: &[u8]) -> Result<(), DomainError> {
        let tx = &self.tx;
        match (table, key) {
            (Table::Ceremonies, Key::Str(k)) => {
                write_table!(tx, CEREMONIES, "write ceremony", |t| {
                    t.insert(k, value)
                        .map(drop)
                        .map_err(|error| store_failure(error, "write ceremony"))
                })
            }
            (Table::LegacyStateMigrations, Key::Str(k)) => {
                write_table!(tx, LEGACY_STATE_MIGRATIONS, "write migration", |t| {
                    t.insert(k, value)
                        .map(drop)
                        .map_err(|error| store_failure(error, "write migration"))
                })
            }
            (Table::Journal, Key::Bytes(k)) => write_table!(tx, JOURNAL, "write journal", |t| {
                t.insert(k, value)
                    .map(drop)
                    .map_err(|error| store_failure(error, "write journal"))
            }),
            (Table::Outbox, Key::Bytes(k)) => write_table!(tx, OUTBOX, "write outbox", |t| {
                t.insert(k, value)
                    .map(drop)
                    .map_err(|error| store_failure(error, "write outbox"))
            }),
            (Table::Publications, Key::Bytes(k)) => {
                write_table!(tx, PUBLICATIONS, "write publication", |t| {
                    t.insert(k, value)
                        .map(drop)
                        .map_err(|error| store_failure(error, "write publication"))
                })
            }
            (table, key) => Err(key_shape_mismatch(table, key.shape())),
        }
    }

    fn commit(self: Box<Self>) -> Result<(), DomainError> {
        self.tx
            .commit()
            .map_err(|error| store_failure(error, "commit"))
    }
}
