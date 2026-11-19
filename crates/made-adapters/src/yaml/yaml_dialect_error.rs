//! [`YamlDialectError`] — why a document could not be read or written.

use std::fmt;

/// The parser's own message, on one line, with its line and column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlDialectError {
    message: String,
}

impl YamlDialectError {
    pub(super) fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for YamlDialectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for YamlDialectError {}
