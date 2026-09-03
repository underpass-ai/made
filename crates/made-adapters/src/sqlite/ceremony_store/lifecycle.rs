use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use made_core::error::DomainError;

use crate::engine::sqlite::SqliteEngine;
use crate::engine::Engine;

use super::SqliteCeremonyStore;

const LEGACY_REDB_HEADER: &[u8] = b"redb";
const LEGACY_STORE_REASON: &str =
    "legacy redb ceremony store detected; convert it with made-mcp v0.2.0 before upgrading";

impl SqliteCeremonyStore {
    /// Open the canonical WAL-mode SQLite store, creating it when absent.
    ///
    /// A legacy Redb path or file is refused before SQLite can create or
    /// overwrite anything beside it. Operators must convert with the last
    /// dual-engine release first.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let path = path.as_ref();
        refuse_legacy_redb(path)?;
        let engine: Arc<dyn Engine> = Arc::new(SqliteEngine::open(path)?);
        Ok(Self { engine })
    }
}

fn refuse_legacy_redb(path: &Path) -> Result<(), DomainError> {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("redb"))
    {
        return legacy_store_error(path);
    }

    let Ok(mut file) = std::fs::File::open(path) else {
        return Ok(());
    };
    let mut header = [0_u8; LEGACY_REDB_HEADER.len()];
    if file.read_exact(&mut header).is_ok() && header == LEGACY_REDB_HEADER {
        return legacy_store_error(path);
    }
    Ok(())
}

fn legacy_store_error(path: &Path) -> Result<(), DomainError> {
    tracing::error!(
        path = %path.display(),
        "legacy Redb ceremony store refused; convert it with made-mcp v0.2.0 before upgrading"
    );
    Err(DomainError::InvariantViolated {
        reason: LEGACY_STORE_REASON,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_legacy_extension_is_refused_without_creating_a_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ceremonies.redb");

        let error = SqliteCeremonyStore::open(&path).unwrap_err();

        assert!(error.to_string().contains("made-mcp v0.2.0"));
        assert!(!path.exists());
    }

    #[test]
    fn a_legacy_header_is_refused_without_modifying_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ceremonies.sqlite3");
        std::fs::write(&path, b"redb legacy bytes").unwrap();
        let before = std::fs::read(&path).unwrap();

        let error = SqliteCeremonyStore::open(&path).unwrap_err();

        assert!(error.to_string().contains("made-mcp v0.2.0"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
