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

use made_core::error::DomainError;

mod bytes_row;
pub mod detect;
mod key;
mod key_shape;
mod read_tx;
pub(crate) mod redb;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;
mod storage_engine;
mod str_row;
mod table;
mod write_tx;

pub(crate) use bytes_row::BytesRow;
pub(crate) use key::Key;
pub(crate) use key_shape::KeyShape;
pub(crate) use read_tx::ReadTx;
pub(crate) use storage_engine::Engine;
pub(crate) use str_row::StrRow;
pub(crate) use table::Table;
pub(crate) use write_tx::WriteTx;

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
