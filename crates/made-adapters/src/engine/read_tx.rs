use made_core::error::DomainError;

use super::{BytesRow, Key, StrRow, Table};

/// A read transaction: a consistent snapshot of every table.
pub(crate) trait ReadTx {
    fn get(&self, table: Table, key: Key<'_>) -> Result<Option<Vec<u8>>, DomainError>;
    fn scan_str(&self, table: Table) -> Result<Vec<StrRow>, DomainError>;
    fn scan_bytes(&self, table: Table) -> Result<Vec<BytesRow>, DomainError>;
    fn scan_bytes_range(
        &self,
        table: Table,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<BytesRow>, DomainError>;
}
