//! The SQLite engine behind the seam. Opt-in via the `sqlite` feature.
//!
//! One SQL table per seam table, keyed by the seam's key shape. Text keys are
//! `TEXT PRIMARY KEY`, byte keys `BLOB PRIMARY KEY`, both `WITHOUT ROWID` so
//! the primary key *is* the storage order and a range scan is an index walk.
//!
//! Ordering matches the seam contract with no collation work, and that is
//! load-bearing rather than convenient: the journal and outbox keys end in a
//! big-endian ordinal so byte order is write order, and SQLite compares BLOBs
//! with `memcmp` — byte by byte, exactly as redb does. Text keys use the
//! default `BINARY` collation, which is also bytewise. An engine that sorted
//! either by locale would hand back a ceremony's history shuffled.
//!
//! What makes this engine worth having: WAL mode. Readers never block the
//! writer, and a second process wanting to write waits for the commit lock
//! instead of being refused, so two agent hosts can hold one store.
//!
//! Durability is `synchronous=FULL`: every commit reaches the disk before it
//! returns, matching the crash contract redb gives.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use made_core::error::DomainError;
use rusqlite::{params, Connection, OptionalExtension};

use super::{
    key_shape_mismatch, scan_shape_mismatch, BytesRow, Engine, Key, KeyShape, ReadTx, StrRow,
    Table, WriteTx,
};

/// How long a transaction waits for another process's commit before giving
/// up. A ceremony step commits in milliseconds; ten seconds means the other
/// side is stuck, not busy.
const BUSY_TIMEOUT: Duration = Duration::from_secs(10);

const ALL_TABLES: [Table; 5] = [
    Table::Ceremonies,
    Table::Journal,
    Table::Outbox,
    Table::Publications,
    Table::LegacyStateMigrations,
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
    // busy_timeout FIRST. Switching the journal mode takes a brief exclusive
    // lock, so two processes opening at the same instant collide there —
    // before WAL is even in effect. Without the timeout already armed, the
    // loser gets SQLITE_BUSY instead of waiting a few milliseconds.
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|error| failure(&error, "set busy timeout"))?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|error| failure(&error, "set journal mode"))?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(|error| failure(&error, "set synchronous"))?;
    Ok(connection)
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

// ------------------------------------------------------------- pooling --

/// A connection borrowed from the engine's pool, returned on drop with any
/// half-open transaction rolled back.
struct Pooled<'e> {
    connection: Option<Connection>,
    pool: &'e Mutex<Vec<Connection>>,
}

impl std::ops::Deref for Pooled<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.connection
            .as_ref()
            .expect("pooled connection is present until drop")
    }
}

impl Drop for Pooled<'_> {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            // A transaction still open here means the owner never committed:
            // roll it back so the connection is clean for the next borrower.
            // Both steps are best effort — nothing sensible can be done about
            // a failure inside drop, and a connection that will not roll back
            // is simply not returned to the pool.
            if !connection.is_autocommit() && connection.execute_batch("ROLLBACK").is_err() {
                return;
            }
            if let Ok(mut pool) = self.pool.lock() {
                pool.push(connection);
            }
        }
    }
}

// ---------------------------------------------------------- statements --

fn check_key(table: Table, key: Key<'_>) -> Result<(), DomainError> {
    if table.key_shape() == key.shape() {
        Ok(())
    } else {
        Err(key_shape_mismatch(table, key.shape()))
    }
}

/// Every seam operation on one connection. Both transaction types delegate
/// here; the difference between them is only which `BEGIN` they issued.
struct Ops<'c> {
    connection: &'c Connection,
}

impl Ops<'_> {
    fn get(&self, table: Table, key: Key<'_>) -> Result<Option<Vec<u8>>, DomainError> {
        check_key(table, key)?;
        let sql = format!("SELECT v FROM \"{table}\" WHERE k = ?1");
        let mut statement = self.prepare(&sql)?;
        let found = match key {
            Key::Str(k) => statement.query_row(params![k], |row| row.get::<_, Vec<u8>>(0)),
            Key::Bytes(k) => statement.query_row(params![k], |row| row.get::<_, Vec<u8>>(0)),
        };
        found
            .optional()
            .map_err(|error| failure(&error, "read row"))
    }

    fn scan_str(&self, table: Table) -> Result<Vec<StrRow>, DomainError> {
        if table.key_shape() != KeyShape::Str {
            return Err(scan_shape_mismatch(table, KeyShape::Str));
        }
        let sql = format!("SELECT k, v FROM \"{table}\" ORDER BY k");
        let mut statement = self.prepare(&sql)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|error| failure(&error, "scan rows"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| failure(&error, "scan rows"))
    }

    fn scan_bytes(&self, table: Table) -> Result<Vec<BytesRow>, DomainError> {
        if table.key_shape() != KeyShape::Bytes {
            return Err(scan_shape_mismatch(table, KeyShape::Bytes));
        }
        let sql = format!("SELECT k, v FROM \"{table}\" ORDER BY k");
        let mut statement = self.prepare(&sql)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|error| failure(&error, "scan rows"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| failure(&error, "scan rows"))
    }

    fn scan_bytes_range(
        &self,
        table: Table,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<BytesRow>, DomainError> {
        if table.key_shape() != KeyShape::Bytes {
            return Err(scan_shape_mismatch(table, KeyShape::Bytes));
        }
        // BETWEEN is inclusive at both ends, which is the seam's contract.
        let sql = format!("SELECT k, v FROM \"{table}\" WHERE k BETWEEN ?1 AND ?2 ORDER BY k");
        let mut statement = self.prepare(&sql)?;
        let rows = statement
            .query_map(params![start, end], |row| {
                Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|error| failure(&error, "scan range"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| failure(&error, "scan range"))
    }

    fn insert(&self, table: Table, key: Key<'_>, value: &[u8]) -> Result<(), DomainError> {
        check_key(table, key)?;
        let sql = format!(
            "INSERT INTO \"{table}\" (k, v) VALUES (?1, ?2) \
             ON CONFLICT (k) DO UPDATE SET v = excluded.v"
        );
        let mut statement = self.prepare(&sql)?;
        let done = match key {
            Key::Str(k) => statement.execute(params![k, value]),
            Key::Bytes(k) => statement.execute(params![k, value]),
        };
        done.map(drop).map_err(|error| failure(&error, "write row"))
    }

    fn prepare(&self, sql: &str) -> Result<rusqlite::CachedStatement<'_>, DomainError> {
        // The statement cache is per connection; with a fixed table set the
        // handful of distinct SQL strings compile once per connection.
        self.connection
            .prepare_cached(sql)
            .map_err(|error| failure(&error, "prepare statement"))
    }
}

// ------------------------------------------------------------ read txn --

struct SqliteRead<'e> {
    connection: Pooled<'e>,
}

impl SqliteRead<'_> {
    fn ops(&self) -> Ops<'_> {
        Ops {
            connection: &self.connection,
        }
    }
}

impl ReadTx for SqliteRead<'_> {
    fn get(&self, table: Table, key: Key<'_>) -> Result<Option<Vec<u8>>, DomainError> {
        self.ops().get(table, key)
    }
    fn scan_str(&self, table: Table) -> Result<Vec<StrRow>, DomainError> {
        self.ops().scan_str(table)
    }
    fn scan_bytes(&self, table: Table) -> Result<Vec<BytesRow>, DomainError> {
        self.ops().scan_bytes(table)
    }
    fn scan_bytes_range(
        &self,
        table: Table,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<BytesRow>, DomainError> {
        self.ops().scan_bytes_range(table, start, end)
    }
}

// ----------------------------------------------------------- write txn --

struct SqliteWrite<'e> {
    connection: Pooled<'e>,
}

impl SqliteWrite<'_> {
    fn ops(&self) -> Ops<'_> {
        Ops {
            connection: &self.connection,
        }
    }
}

impl ReadTx for SqliteWrite<'_> {
    fn get(&self, table: Table, key: Key<'_>) -> Result<Option<Vec<u8>>, DomainError> {
        self.ops().get(table, key)
    }
    fn scan_str(&self, table: Table) -> Result<Vec<StrRow>, DomainError> {
        self.ops().scan_str(table)
    }
    fn scan_bytes(&self, table: Table) -> Result<Vec<BytesRow>, DomainError> {
        self.ops().scan_bytes(table)
    }
    fn scan_bytes_range(
        &self,
        table: Table,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<BytesRow>, DomainError> {
        self.ops().scan_bytes_range(table, start, end)
    }
}

impl WriteTx for SqliteWrite<'_> {
    fn insert(&mut self, table: Table, key: Key<'_>, value: &[u8]) -> Result<(), DomainError> {
        self.ops().insert(table, key, value)
    }

    fn commit(self: Box<Self>) -> Result<(), DomainError> {
        // After COMMIT the connection is back in autocommit, so the pooled
        // drop that follows returns it clean instead of rolling anything back.
        self.connection
            .execute_batch("COMMIT")
            .map_err(|error| failure(&error, "commit"))
    }
}

// ---------------------------------------------------------------- errors --

fn poisoned() -> DomainError {
    tracing::error!("embedded sqlite connection pool is poisoned");
    DomainError::InvariantViolated {
        reason: "sqlite: connection pool is poisoned",
    }
}

/// Maps a rusqlite failure the way the redb adapter maps its own: the runtime
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
