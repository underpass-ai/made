use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::value_objects::CeremonyId;

use crate::engine::sqlite::SqliteEngine;
use crate::engine::{Engine, Key, Table};
use crate::sqlite::keys::scoped;

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
        let store = Self { engine };
        let stranded = store.legacy_instances_without_a_stream()?;
        if stranded > 0 {
            tracing::warn!(
                path = %path.display(),
                count = stranded,
                "ceremony instances from an earlier store have no event stream and are not \
                 visible to the event-sourced engine; run `made-mcp migrate-store <path>` to \
                 import them, which copies the store, verifies every session and keeps the \
                 original beside the new file"
            );
        }
        Ok(store)
    }

    /// How many instances the legacy `ceremony_instances` table holds
    /// that have no stream in `ceremony_events`.
    ///
    /// Stores written before ceremonies became event streams (v0.3.0
    /// and earlier) kept an instance and a journal, not a stream. The
    /// event-sourced engine reads streams only, so those instances are
    /// there but unreachable until `made-mcp migrate-store` imports
    /// them. Counted at open so an operator is told, rather than
    /// finding an empty list and a full file.
    pub fn legacy_instances_without_a_stream(&self) -> Result<usize, DomainError> {
        let tx = self.engine.begin_read()?;
        let mut stranded = 0;
        for (id, _) in tx.scan_str(Table::Ceremonies)? {
            let ceremony_id = CeremonyId::new(id)?;
            let opening = scoped(&ceremony_id, 1);
            if tx.get(Table::Events, Key::Bytes(&opening))?.is_none() {
                stranded += 1;
            }
        }
        Ok(stranded)
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
    use made_app::services::SessionStream;
    use made_core::ports::{
        CeremonyEventStorePort, CeremonySnapshotStorePort, NoopCeremonyEventSubscriber,
    };
    use made_core::value_objects::{AuditActor, AuditActorKind, CeremonyContext, StreamVersion};
    use time::OffsetDateTime;

    use super::super::legacy_store_fixture::{definition, write_legacy_instance};
    use super::*;
    use made_core::entities::CeremonyInstance;

    /// An instance the old path stored is counted at open and is not a
    /// session the event-sourced path can load: no stream, no
    /// snapshot, `NotFound`.
    #[tokio::test]
    async fn a_legacy_instance_without_a_stream_is_counted_and_not_loadable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ceremonies.sqlite3");
        let id = CeremonyId::new("stranded-1").unwrap();
        {
            let store = SqliteCeremonyStore::open(&path).unwrap();
            assert_eq!(store.legacy_instances_without_a_stream().unwrap(), 0);
            write_legacy_instance(&store, &id);
        }

        let store = Arc::new(SqliteCeremonyStore::open(&path).unwrap());
        assert_eq!(store.legacy_instances_without_a_stream().unwrap(), 1);
        assert_eq!(store.head(&id).await.unwrap(), StreamVersion::EMPTY);
        assert_eq!(store.latest(&id).await.unwrap(), None);
        let loaded = SessionStream::new(
            store.clone(),
            store.clone(),
            Arc::new(NoopCeremonyEventSubscriber),
        )
        .load(&id)
        .await;
        assert!(
            matches!(
                loaded,
                Err(DomainError::NotFound {
                    what: "ceremony_instance"
                })
            ),
            "a stranded instance must not load as a session, got {loaded:?}"
        );
    }

    /// A ceremony opened as a stream is not a stranded instance, even
    /// with a legacy row beside it.
    #[tokio::test]
    async fn an_instance_with_a_stream_is_not_counted() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ceremonies.sqlite3");
        let store = Arc::new(SqliteCeremonyStore::open(&path).unwrap());
        let streamed = CeremonyId::new("streamed-1").unwrap();
        let stream = SessionStream::new(
            store.clone(),
            store.clone(),
            Arc::new(NoopCeremonyEventSubscriber),
        );
        stream
            .open(
                CeremonyInstance::decide_start(
                    streamed.clone(),
                    &definition(),
                    CeremonyContext::empty(),
                    None,
                    OffsetDateTime::UNIX_EPOCH,
                ),
                AuditActor::new("test", AuditActorKind::Service, None).unwrap(),
                OffsetDateTime::UNIX_EPOCH,
            )
            .await
            .unwrap();
        write_legacy_instance(&store, &CeremonyId::new("stranded-2").unwrap());

        assert_eq!(store.legacy_instances_without_a_stream().unwrap(), 1);
        assert!(stream.load(&streamed).await.is_ok());
    }

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
