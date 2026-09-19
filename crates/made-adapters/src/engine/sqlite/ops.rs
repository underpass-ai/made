use made_core::error::DomainError;
use rusqlite::{params, Connection, OptionalExtension};

use crate::engine::{scan_shape_mismatch, BytesRow, Key, KeyShape, StrRow, Table};

use super::{check_key, failure};

pub(super) struct Ops<'c> {
    pub(super) connection: &'c Connection,
}

fn checked_scan_limit(limit: usize) -> Result<i64, DomainError> {
    i64::try_from(limit).map_err(|_| DomainError::OutOfRange {
        field: "embedded_store.scan_limit",
        value: limit as f64,
        min: 0.0,
        max: i64::MAX as f64,
    })
}

impl Ops<'_> {
    pub(super) fn get(&self, table: Table, key: Key<'_>) -> Result<Option<Vec<u8>>, DomainError> {
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

    pub(super) fn scan_str(&self, table: Table) -> Result<Vec<StrRow>, DomainError> {
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

    pub(super) fn scan_str_page(
        &self,
        table: Table,
        after: Option<&str>,
        prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<StrRow>, DomainError> {
        if table.key_shape() != KeyShape::Str {
            return Err(scan_shape_mismatch(table, KeyShape::Str));
        }
        let sql = format!(
            "SELECT k, v FROM \"{table}\" \
             WHERE (?1 IS NULL OR k > ?1) \
               AND (?2 IS NULL OR substr(k, 1, length(?2)) = ?2) \
             ORDER BY k LIMIT ?3"
        );
        let limit = checked_scan_limit(limit)?;
        let mut statement = self.prepare(&sql)?;
        let rows = statement
            .query_map(params![after, prefix, limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|error| failure(&error, "scan string page"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| failure(&error, "scan string page"))
    }

    pub(super) fn scan_str_after(
        &self,
        table: Table,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<StrRow>, DomainError> {
        if table.key_shape() != KeyShape::Str {
            return Err(scan_shape_mismatch(table, KeyShape::Str));
        }
        let limit = checked_scan_limit(limit)?;
        let sql = format!("SELECT k, v FROM \"{table}\" WHERE k > ?1 ORDER BY k LIMIT ?2");
        let after = after.unwrap_or_default();
        let mut statement = self.prepare(&sql)?;
        let rows = statement
            .query_map(params![after, limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|error| failure(&error, "scan bounded rows"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| failure(&error, "scan bounded rows"))
    }

    pub(super) fn scan_bytes(&self, table: Table) -> Result<Vec<BytesRow>, DomainError> {
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

    pub(super) fn scan_byte_keys_page(
        &self,
        table: Table,
        after: Option<&[u8]>,
        limit: usize,
    ) -> Result<Vec<Vec<u8>>, DomainError> {
        if table.key_shape() != KeyShape::Bytes {
            return Err(scan_shape_mismatch(table, KeyShape::Bytes));
        }
        let sql =
            format!("SELECT k FROM \"{table}\" WHERE (?1 IS NULL OR k > ?1) ORDER BY k LIMIT ?2");
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.prepare(&sql)?;
        let rows = statement
            .query_map(params![after, limit], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|error| failure(&error, "scan byte keys page"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| failure(&error, "scan byte keys page"))
    }

    pub(super) fn scan_bytes_range(
        &self,
        table: Table,
        start: &[u8],
        end: &[u8],
    ) -> Result<Vec<BytesRow>, DomainError> {
        if table.key_shape() != KeyShape::Bytes {
            return Err(scan_shape_mismatch(table, KeyShape::Bytes));
        }
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

    pub(super) fn insert(
        &self,
        table: Table,
        key: Key<'_>,
        value: &[u8],
    ) -> Result<(), DomainError> {
        check_key(table, key)?;
        let sql = format!("INSERT INTO \"{table}\" (k, v) VALUES (?1, ?2) ON CONFLICT (k) DO UPDATE SET v = excluded.v");
        let mut statement = self.prepare(&sql)?;
        let done = match key {
            Key::Str(k) => statement.execute(params![k, value]),
            Key::Bytes(k) => statement.execute(params![k, value]),
        };
        done.map(drop).map_err(|error| failure(&error, "write row"))
    }

    pub(super) fn remove(&self, table: Table, key: Key<'_>) -> Result<(), DomainError> {
        check_key(table, key)?;
        let sql = format!("DELETE FROM \"{table}\" WHERE k = ?1");
        let mut statement = self.prepare(&sql)?;
        let done = match key {
            Key::Str(k) => statement.execute(params![k]),
            Key::Bytes(k) => statement.execute(params![k]),
        };
        done.map(drop)
            .map_err(|error| failure(&error, "delete row"))
    }

    fn prepare(&self, sql: &str) -> Result<rusqlite::CachedStatement<'_>, DomainError> {
        self.connection
            .prepare_cached(sql)
            .map_err(|error| failure(&error, "prepare statement"))
    }
}
