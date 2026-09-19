use made_core::error::DomainError;

use crate::engine::{BytesRow, Key, ReadTx, StrRow, Table};

use super::{Ops, Pooled};

pub(super) struct SqliteRead<'e> {
    pub(super) connection: Pooled<'e>,
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
    fn scan_str_page(
        &self,
        table: Table,
        after: Option<&str>,
        prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<StrRow>, DomainError> {
        self.ops().scan_str_page(table, after, prefix, limit)
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
