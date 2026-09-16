use made_core::error::DomainError;

use super::{Key, ReadTx, Table};

/// A write transaction whose writes become durable only on commit.
pub(crate) trait WriteTx: ReadTx {
    fn insert(&mut self, table: Table, key: Key<'_>, value: &[u8]) -> Result<(), DomainError>;
    /// Drop one row; a key that is not there is not an error.
    fn remove(&mut self, table: Table, key: Key<'_>) -> Result<(), DomainError>;
    fn commit(self: Box<Self>) -> Result<(), DomainError>;
}
