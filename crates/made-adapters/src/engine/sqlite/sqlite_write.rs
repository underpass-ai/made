use made_core::error::DomainError;

use crate::engine::{BytesRow, Key, ReadTx, StrRow, Table, WriteTx};

use super::{failure, Ops, Pooled};

pub(super) struct SqliteWrite<'e> {
    pub(super) connection: Pooled<'e>,
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
    fn scan_byte_keys_page(
        &self,
        table: Table,
        after: Option<&[u8]>,
        limit: usize,
    ) -> Result<Vec<Vec<u8>>, DomainError> {
        self.ops().scan_byte_keys_page(table, after, limit)
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
    fn remove(&mut self, table: Table, key: Key<'_>) -> Result<(), DomainError> {
        self.ops().remove(table, key)
    }
    fn commit(self: Box<Self>) -> Result<(), DomainError> {
        self.connection
            .execute_batch("COMMIT")
            .map_err(|error| failure(&error, "commit"))
    }
}
