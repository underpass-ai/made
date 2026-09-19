//! The SQLite engine behind the canonical embedded store.
//!
//! One SQL table per seam table, keyed by the seam's key shape. Text keys are
//! `TEXT PRIMARY KEY`, byte keys `BLOB PRIMARY KEY`, both `WITHOUT ROWID` so
//! the primary key *is* the storage order and a range scan is an index walk.
//!
//! Ordering matches the seam contract with no collation work, and that is
//! load-bearing rather than convenient: the event keys end in a
//! big-endian ordinal so byte order is write order, and SQLite compares BLOBs
//! with `memcmp` — byte by byte. Text keys use the default `BINARY`
//! collation, which is also bytewise. An engine that sorted
//! either by locale would hand back a ceremony's history shuffled.
//!
//! What makes this engine worth having: WAL mode. Readers never block the
//! writer, and a second process wanting to write waits for the commit lock
//! instead of being refused, so two agent hosts can hold one store.
//!
//! Durability is `synchronous=FULL`: every commit reaches the disk before it
//! returns.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use made_core::error::DomainError;
use rusqlite::Connection;

use super::{key_shape_mismatch, Engine, Key, KeyShape, ReadTx, Table, WriteTx};

mod ops;
mod pooled;
mod sqlite_read;
mod sqlite_write;

use ops::Ops;
use pooled::Pooled;
use sqlite_read::SqliteRead;
use sqlite_write::SqliteWrite;

/// How long a transaction waits for another process's commit before giving
/// up. A ceremony step commits in milliseconds; ten seconds means the other
/// side is stuck, not busy.
const BUSY_TIMEOUT: Duration = Duration::from_secs(10);

/// The tables this engine creates when a store is opened. A store
/// written by an earlier version may hold others — `outbox`,
/// `state_migrations` — and they are left exactly where they are:
/// nothing here writes them, and dropping a table an operator can
/// still read is not this command's to do.
const ALL_TABLES: [Table; 21] = [
    Table::Ceremonies,
    Table::Journal,
    Table::Publications,
    Table::Events,
    Table::EventLog,
    Table::Snapshots,
    Table::Meta,
    Table::EventCursors,
    Table::EventCursorQuarantine,
    Table::MemoryWrites,
    Table::ExecutionOperations,
    Table::ExecutionIntents,
    Table::ExecutionReceipts,
    Table::Councils,
    Table::CouncilAgents,
    Table::CouncilContracts,
    Table::CouncilDeliberations,
    Table::CouncilStatistics,
    Table::CouncilJournal,
    Table::CouncilJournalIds,
    Table::CouncilJournalCursors,
];

/// One open SQLite file, with a small pool so concurrent blocking tasks each
/// get their own snapshot.
#[derive(Debug)]
pub(crate) struct SqliteEngine {
    path: PathBuf,
    pool: Mutex<Vec<Connection>>,
}

impl SqliteEngine {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let path = path.as_ref().to_path_buf();
        let connection = open_connection(&path)?;
        create_tables(&connection)?;
        Ok(Self {
            path,
            pool: Mutex::new(vec![connection]),
        })
    }

    fn take_connection(&self) -> Result<Pooled<'_>, DomainError> {
        let reused = self.pool.lock().map_err(|_| poisoned())?.pop();
        let connection = match reused {
            Some(connection) => connection,
            None => open_connection(&self.path)?,
        };
        Ok(Pooled {
            connection: Some(connection),
            pool: &self.pool,
        })
    }
}

impl Engine for SqliteEngine {
    fn begin_read(&self) -> Result<Box<dyn ReadTx + '_>, DomainError> {
        let connection = self.take_connection()?;
        // A deferred BEGIN: the snapshot is taken at the first read and held
        // until the transaction ends, which is what lets a multi-step read of
        // one ceremony's journal see one consistent store.
        connection
            .execute_batch("BEGIN")
            .map_err(|error| failure(&error, "begin read"))?;
        Ok(Box::new(SqliteRead { connection }))
    }

    fn begin_write(&self) -> Result<Box<dyn WriteTx + '_>, DomainError> {
        let connection = self.take_connection()?;
        // IMMEDIATE takes the write lock now, waiting up to BUSY_TIMEOUT for
        // another process to finish committing. A deferred BEGIN would take
        // it on the first write and could be refused after reads were already
        // done — the classic upgrade deadlock. This is the line that lets a
        // second process write rather than be turned away.
        connection
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(|error| failure(&error, "begin write"))?;
        Ok(Box::new(SqliteWrite { connection }))
    }
}

fn open_connection(path: &Path) -> Result<Connection, DomainError> {
    let connection = Connection::open(path).map_err(|error| failure(&error, "open database"))?;
    // busy_timeout first, so every ordinary lock contention becomes a wait.
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|error| failure(&error, "set busy timeout"))?;
    enter_wal(&connection)?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(|error| failure(&error, "set synchronous"))?;
    Ok(connection)
}

/// Puts the connection in WAL mode, waiting out the one case `busy_timeout`
/// does not cover.
///
/// SQLite will not move a database into or out of WAL while another
/// connection has it open: that conversion takes an exclusive lock, and the
/// busy handler is not consulted for it. So two processes opening a *fresh*
/// store at the same instant both try to convert, and the loser gets
/// SQLITE_BUSY immediately however long its timeout is.
///
/// The wait is all that is needed, because the winner's conversion is what
/// resolves it: once the file is in WAL, this pragma is a no-op that takes no
/// exclusive lock, so the retry succeeds the moment the other side finishes.
/// After the first open of a store's life the loop never runs twice.
fn enter_wal(connection: &Connection) -> Result<(), DomainError> {
    let deadline = std::time::Instant::now() + BUSY_TIMEOUT;
    let mut backoff = Duration::from_millis(2);
    loop {
        match connection.pragma_update(None, "journal_mode", "WAL") {
            Ok(()) => return Ok(()),
            Err(error) if is_busy(&error) && std::time::Instant::now() < deadline => {
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(Duration::from_millis(64));
            }
            Err(error) => return Err(failure(&error, "set journal mode")),
        }
    }
}

fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
}

fn create_tables(connection: &Connection) -> Result<(), DomainError> {
    use std::fmt::Write as _;

    let mut ddl = String::new();
    for table in ALL_TABLES {
        let key_column = match table.key_shape() {
            KeyShape::Str => "k TEXT NOT NULL",
            KeyShape::Bytes => "k BLOB NOT NULL",
        };
        // WITHOUT ROWID: the primary key is the storage order, so a scope
        // scan is a range over the key rather than a lookup per row.
        writeln!(
            ddl,
            "CREATE TABLE IF NOT EXISTS \"{table}\" ({key_column}, v BLOB NOT NULL, \
             PRIMARY KEY (k)) WITHOUT ROWID;"
        )
        .expect("writing to a String cannot fail");
    }
    connection
        .execute_batch(&ddl)
        .map_err(|error| failure(&error, "create tables"))
}

// ---------------------------------------------------------- statements --

fn check_key(table: Table, key: Key<'_>) -> Result<(), DomainError> {
    if table.key_shape() == key.shape() {
        Ok(())
    } else {
        Err(key_shape_mismatch(table, key.shape()))
    }
}

// ---------------------------------------------------------------- errors --

fn poisoned() -> DomainError {
    tracing::error!("embedded sqlite connection pool is poisoned");
    DomainError::InvariantViolated {
        reason: "sqlite: connection pool is poisoned",
    }
}

/// Maps a rusqlite failure into the stable domain error used by the runtime:
/// detail goes to the structured log, and a small stable set of static
/// reasons crosses into the domain.
fn failure(error: &rusqlite::Error, op: &'static str) -> DomainError {
    let rendered = error.to_string();
    tracing::error!(error = %rendered, operation = op, "sqlite operation failed");
    if rendered.contains("database is locked") {
        return DomainError::InvariantViolated {
            reason: "sqlite: the store is busy and did not free within the wait",
        };
    }
    DomainError::InvariantViolated {
        reason: "sqlite: persistence backend failed",
    }
}
