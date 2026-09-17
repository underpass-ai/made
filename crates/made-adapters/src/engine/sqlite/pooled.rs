use std::sync::Mutex;

use rusqlite::Connection;

/// A connection borrowed from the engine's pool, returned on drop.
pub(super) struct Pooled<'e> {
    pub(super) connection: Option<Connection>,
    pub(super) pool: &'e Mutex<Vec<Connection>>,
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
            if !connection.is_autocommit() && connection.execute_batch("ROLLBACK").is_err() {
                return;
            }
            if let Ok(mut pool) = self.pool.lock() {
                pool.push(connection);
            }
        }
    }
}
