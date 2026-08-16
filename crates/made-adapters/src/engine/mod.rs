//! The storage seam.
//!
//! Everything the ceremony store needs from a storage engine, and nothing an
//! engine needs to know about ceremonies: five key-to-bytes maps, two key
//! shapes, transactions over them. The store's logic — revision guards,
//! outbox claiming, journal ordering — is written once against this and never
//! names an engine type.
//!
//! The seam is deliberately narrow. Every method corresponds to an operation
//! the store already performed against redb, and no more: point get, insert,
//! a full ordered scan, and a scan of an inclusive byte range. There is no
//! remove and no count because the ceremony store does neither — a seam wider
//! than its callers is a second engine's worth of surface nobody asked for,
//! and every method of it has to be right in both engines forever.
//!
//! Two contracts the store relies on, stated here because a second engine has
//! to honour them:
//!
//!   * **Rows come back in ascending key order, compared byte by byte.** The
//!     journal and the outbox key on a ceremony id, a `0x00` separator and a
//!     big-endian ordinal precisely so byte order is write order
//!     ([`super::redb::keys`]). An engine that ordered text keys by locale, or
//!     blobs by anything but memcmp, would return a ceremony's history
//!     shuffled.
//!   * **A table that has never been written reads as empty**, never as
//!     missing.
//!
//! Scans return `Vec` rather than an iterator: every caller collected before
//! this seam existed, so nothing is lost, and it keeps the traits object-safe
//! without a lifetime tying a row to its transaction.

use std::fmt;

use made_core::error::DomainError;

pub(crate) mod redb;

/// The tables an embedded ceremony store consists of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Table {
    /// Ceremony state: `ceremony_id -> StoredCeremony`.
    Ceremonies,
    /// Audit journal: `(ceremony_id, 0x00, ordinal) -> AuditRecord`.
    Journal,
    /// Outbox: `(ceremony_id, 0x00, ordinal) -> StoredOutboxMessage`.
    Outbox,
    /// Published definitions: `(len, name, version) -> SealedDefinition`.
    Publications,
    /// Receipts of the pre-rename Choreographer import.
    LegacyStateMigrations,
}

impl Table {
    /// The key shape this table is defined with. A call carrying another
    /// shape is a programming error inside this crate, and the engine reports
    /// it as one instead of guessing.
    pub(crate) const fn key_shape(self) -> KeyShape {
        match self {
            Table::Ceremonies | Table::LegacyStateMigrations => KeyShape::Str,
            Table::Journal | Table::Outbox | Table::Publications => KeyShape::Bytes,
        }
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Table::Ceremonies => "ceremony_instances",
            Table::Journal => "audit_journal",
            Table::Outbox => "outbox",
            Table::Publications => "published_definitions",
            Table::LegacyStateMigrations => "state_migrations",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyShape {
    Str,
    Bytes,
}

/// A borrowed key in one of the two shapes the tables use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key<'a> {
    Str(&'a str),
    Bytes(&'a [u8]),
}

impl Key<'_> {
    pub(crate) const fn shape(&self) -> KeyShape {
        match self {
            Key::Str(_) => KeyShape::Str,
            Key::Bytes(_) => KeyShape::Bytes,
        }
    }
}

/// A row from a `Str`-keyed table.
pub(crate) type StrRow = (String, Vec<u8>);
/// A row from a `Bytes`-keyed table.
pub(crate) type BytesRow = (Vec<u8>, Vec<u8>);

/// A read transaction: a consistent snapshot of every table.
pub(crate) trait ReadTx {
    fn get(&self, table: Table, key: Key<'_>) -> Result<Option<Vec<u8>>, DomainError>;

    /// Every row of a `Str`-keyed table, ascending.
    fn scan_str(&self, table: Table) -> Result<Vec<StrRow>, DomainError>;

    /// Every row of a `Bytes`-keyed table, ascending by memcmp.
    fn scan_bytes(&self, table: Table) -> Result<Vec<BytesRow>, DomainError>;

    /// Rows of a `Bytes`-keyed table in `[start, end]`, ascending — **both
    /// ends inclusive**. This is the scope scan: every journal or outbox
    /// record of one ceremony.
    ///
    /// Inclusive because `keys::scope_range` names the upper bound as the
    /// ordinal `u64::MAX`, and that record is inside the scope, not past it.
    /// A half-open range would drop it — a row lost only at a boundary no
    /// test reaches, which is the worst kind to get wrong.
    fn scan_bytes_range(
        &self,
        table: Table,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<BytesRow>, DomainError>;
}

/// A write transaction. Reads see this transaction's own writes; nothing is
/// durable until [`commit`](WriteTx::commit) returns, and dropping the
/// transaction discards everything it did.
pub(crate) trait WriteTx: ReadTx {
    fn insert(&mut self, table: Table, key: Key<'_>, value: &[u8]) -> Result<(), DomainError>;

    fn commit(self: Box<Self>) -> Result<(), DomainError>;
}

/// One opened ceremony store, shareable across tasks.
pub(crate) trait Engine: fmt::Debug + Send + Sync {
    fn begin_read(&self) -> Result<Box<dyn ReadTx + '_>, DomainError>;
    fn begin_write(&self) -> Result<Box<dyn WriteTx + '_>, DomainError>;
}

pub(crate) fn key_shape_mismatch(table: Table, key: KeyShape) -> DomainError {
    tracing::error!(
        table = %table,
        expected = ?table.key_shape(),
        got = ?key,
        "embedded store called with the wrong key shape"
    );
    DomainError::InvariantViolated {
        reason: "embedded store: table called with the wrong key shape",
    }
}

pub(crate) fn scan_shape_mismatch(table: Table, wanted: KeyShape) -> DomainError {
    tracing::error!(
        table = %table,
        expected = ?table.key_shape(),
        got = ?wanted,
        "embedded store scanned with the wrong key shape"
    );
    DomainError::InvariantViolated {
        reason: "embedded store: table scanned with the wrong key shape",
    }
}
