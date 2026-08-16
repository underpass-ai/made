//! Which engine wrote a store file.
//!
//! MADE's embedded store is one file at a path the operator names, not a
//! directory this crate lays out, so there is nowhere to stamp a format
//! marker without inventing one. There is no need to: both engines announce
//! themselves in their first bytes, and reading them is both cheaper and
//! harder to get wrong than a marker that can be moved, copied or lost
//! separately from the file it describes.
//!
//! What this buys is the property that matters: **an existing store is always
//! opened by the engine that wrote it**. Not by configuration, not by a file
//! extension anyone can rename — by what the bytes say.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use made_core::error::DomainError;

/// `"SQLite format 3\0"`, the first sixteen bytes of every SQLite database.
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";
/// redb's own file identifier.
const REDB_MAGIC: &[u8] = b"redb";

/// The engine a ceremony store runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageEngine {
    /// Pure Rust, one file, one process at a time. The default.
    Redb,
    /// WAL-mode SQLite: several processes may hold the same store. Opt-in
    /// through the `sqlite` cargo feature; a binary built without it still
    /// recognises the format and refuses it by name.
    Sqlite,
}

impl StorageEngine {
    /// The name an operator writes in `MADE_MCP_ENGINE` and reads in an error.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            StorageEngine::Redb => "redb",
            StorageEngine::Sqlite => "sqlite",
        }
    }

    /// Whether this build can open the engine, as opposed to merely name it.
    #[must_use]
    pub const fn is_compiled(self) -> bool {
        match self {
            StorageEngine::Redb => true,
            StorageEngine::Sqlite => cfg!(feature = "sqlite"),
        }
    }

    /// Parses an engine name the way the environment variable spells it.
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "redb" => Ok(StorageEngine::Redb),
            "sqlite" => Ok(StorageEngine::Sqlite),
            other => {
                tracing::error!(value = other, "unknown storage engine requested");
                Err(DomainError::InvariantViolated {
                    reason: "unknown storage engine; expected `redb` or `sqlite`",
                })
            }
        }
    }
}

impl std::fmt::Display for StorageEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The engine that wrote `path`, or `None` if there is no store there yet.
///
/// A file that exists but announces neither format is an error rather than a
/// guess: opening it with either engine would either fail confusingly or, far
/// worse, treat somebody else's file as an empty store and write into it.
pub fn engine_of(path: &Path) -> Result<Option<StorageEngine>, DomainError> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            tracing::error!(path = %path.display(), error = %error, "could not read the store file");
            return Err(DomainError::InvariantViolated {
                reason: "the ceremony store file could not be read",
            });
        }
    };

    let mut header = [0u8; 16];
    let read = file.read(&mut header).map_err(|error| {
        tracing::error!(path = %path.display(), error = %error, "could not read the store header");
        DomainError::InvariantViolated {
            reason: "the ceremony store header could not be read",
        }
    })?;
    let header = &header[..read];

    // An empty file is a store nobody has written yet: several tools create
    // one by touching a path, and refusing that would be refusing a fresh
    // start.
    if header.is_empty() {
        return Ok(None);
    }
    if header.starts_with(SQLITE_MAGIC) {
        return Ok(Some(StorageEngine::Sqlite));
    }
    if header.starts_with(REDB_MAGIC) {
        return Ok(Some(StorageEngine::Redb));
    }

    tracing::error!(
        path = %path.display(),
        "the file at the ceremony store path is neither a redb nor a SQLite database"
    );
    Err(DomainError::InvariantViolated {
        reason: "the ceremony store path holds a file that is not a ceremony store",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn file_with(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::TempDir::new().expect("a temporary directory");
        let path = directory.path().join("store");
        let mut file = File::create(&path).expect("create");
        file.write_all(bytes).expect("write");
        (directory, path)
    }

    #[test]
    fn a_missing_file_is_a_store_nobody_has_written_yet() {
        let directory = tempfile::TempDir::new().expect("a temporary directory");
        assert_eq!(engine_of(&directory.path().join("absent")).unwrap(), None);
    }

    #[test]
    fn an_empty_file_is_also_a_fresh_start() {
        // Several tools create a path by touching it; refusing that would be
        // refusing a legitimate first run.
        let (_directory, path) = file_with(b"");
        assert_eq!(engine_of(&path).unwrap(), None);
    }

    #[test]
    fn each_engine_is_recognised_by_its_own_header() {
        let (_d1, redb) = file_with(b"redb\x1a\x0a\xa9\x0d\x0a\x05\x00\x00");
        assert_eq!(engine_of(&redb).unwrap(), Some(StorageEngine::Redb));

        let (_d2, sqlite) = file_with(b"SQLite format 3\0\x10\x00\x02\x02");
        assert_eq!(engine_of(&sqlite).unwrap(), Some(StorageEngine::Sqlite));
    }

    #[test]
    fn a_file_that_is_not_a_store_is_refused_rather_than_guessed() {
        // Opening it with either engine would either fail confusingly or, far
        // worse, treat somebody else's file as an empty store and write to it.
        let (_directory, path) = file_with(b"# my notes\nnot a database at all\n");
        let error = engine_of(&path).expect_err("a text file is not a store");
        assert!(
            error.to_string().contains("not a ceremony store"),
            "{error}"
        );
    }

    #[test]
    fn engine_names_are_case_insensitive_and_trimmed() {
        assert_eq!(StorageEngine::parse("redb").unwrap(), StorageEngine::Redb);
        assert_eq!(
            StorageEngine::parse(" SQLite ").unwrap(),
            StorageEngine::Sqlite
        );
        let error = StorageEngine::parse("postgres").expect_err("not an embedded engine");
        assert!(error.to_string().contains("`redb` or `sqlite`"), "{error}");
    }
}
