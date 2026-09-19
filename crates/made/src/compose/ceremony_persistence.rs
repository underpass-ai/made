use std::sync::Arc;

use made_adapters::config::{MemorySelection, ServiceConfig};
use made_adapters::memory::{
    ForgetfulMemory, InMemoryBudgetLedgerStore, InMemoryCeremonyDefinitionPublications,
    InMemoryCeremonyEventCursor, InMemoryCeremonyEventStore, InMemoryExecutionReceiptStore,
};
use made_adapters::postgres::{PostgresCeremonyStore, PostgresPool};
use made_adapters::sqlite::{SqliteBudgetLedgerStore, SqliteCeremonyStore};
use made_core::ports::{
    BudgetLedgerStorePort, CeremonyDefinitionPublicationPort, CeremonyEventCursorPort,
    CeremonyEventStorePort, CeremonySnapshotStorePort, ExecutionReceiptStorePort, MemoryReaderPort,
    MemoryWriterPort,
};
use tracing::{info, warn};

use crate::ComposeError;

/// Ceremony state and session memory selected as one composition decision.
pub(super) struct CeremonyPersistence {
    pub(super) events: Arc<dyn CeremonyEventStorePort>,
    pub(super) cursors: Arc<dyn CeremonyEventCursorPort>,
    pub(super) snapshots: Arc<dyn CeremonySnapshotStorePort>,
    pub(super) publications: Arc<dyn CeremonyDefinitionPublicationPort>,
    pub(super) memory_writer: Arc<dyn MemoryWriterPort>,
    pub(super) memory_reader: Arc<dyn MemoryReaderPort>,
    pub(super) receipts: Arc<dyn ExecutionReceiptStorePort>,
    pub(super) budgets: Arc<dyn BudgetLedgerStorePort>,
}

pub(super) fn wire(
    config: &ServiceConfig,
    postgres: Option<&PostgresPool>,
) -> Result<CeremonyPersistence, ComposeError> {
    if let Some(pool) = postgres {
        validate_postgres_selection(config)?;
        return Ok(postgres_persistence(pool, config.memory));
    }
    match config.ceremony_store_path.as_deref() {
        Some(path) => sqlite_persistence(path, config.memory),
        None if config.memory == MemorySelection::Sqlite => Err(ComposeError::Memory(
            "MADE_MEMORY=sqlite requires MADE_CEREMONY_STORE_PATH".to_owned(),
        )),
        None => {
            warn!(
                "MADE_CEREMONY_STORE_PATH is unset: ceremony state is held in memory. Step \
                 leases, idempotency keys and pending human guards will not survive a restart."
            );
            if config.memory == MemorySelection::None {
                info!("session memory is disabled explicitly with MADE_MEMORY=none");
            } else {
                warn!(
                    "session memory has no durable store path and will remember nothing; set \
                     MADE_CEREMONY_STORE_PATH or choose MADE_MEMORY=none explicitly"
                );
            }
            let store = Arc::new(InMemoryCeremonyEventStore::new());
            let memory = Arc::new(ForgetfulMemory::new());
            Ok(CeremonyPersistence {
                events: store.clone(),
                cursors: Arc::new(InMemoryCeremonyEventCursor::new()),
                snapshots: store,
                publications: Arc::new(InMemoryCeremonyDefinitionPublications::new()),
                memory_writer: memory.clone(),
                memory_reader: memory,
                receipts: Arc::new(InMemoryExecutionReceiptStore::new()),
                budgets: Arc::new(InMemoryBudgetLedgerStore::new()),
            })
        }
    }
}

fn validate_postgres_selection(config: &ServiceConfig) -> Result<(), ComposeError> {
    if config.ceremony_store_path.is_some() {
        return Err(ComposeError::CeremonyStore(
            "MADE_POSTGRES_URL and MADE_CEREMONY_STORE_PATH select different ceremony stores"
                .to_owned(),
        ));
    }
    if config.memory == MemorySelection::Sqlite {
        return Err(ComposeError::Memory(
            "MADE_MEMORY=sqlite cannot be combined with MADE_POSTGRES_URL".to_owned(),
        ));
    }
    Ok(())
}

fn postgres_persistence(
    pool: &PostgresPool,
    memory_selection: MemorySelection,
) -> CeremonyPersistence {
    let store = Arc::new(PostgresCeremonyStore::new(pool.clone()));
    info!(
        "ceremony state, publications, cursors, execution receipts and budgets are durable in Postgres"
    );
    let (memory_writer, memory_reader): (Arc<dyn MemoryWriterPort>, Arc<dyn MemoryReaderPort>) =
        if memory_selection == MemorySelection::None {
            info!("session memory is disabled explicitly with MADE_MEMORY=none");
            let memory = Arc::new(ForgetfulMemory::new());
            (memory.clone(), memory)
        } else {
            info!("session memory is durable in the ceremony Postgres store");
            (store.clone(), store.clone())
        };
    CeremonyPersistence {
        events: store.clone(),
        cursors: store.clone(),
        snapshots: store.clone(),
        publications: store.clone(),
        memory_writer,
        memory_reader,
        receipts: store.clone(),
        budgets: store,
    }
}

fn sqlite_persistence(
    path: &str,
    memory_selection: MemorySelection,
) -> Result<CeremonyPersistence, ComposeError> {
    let store = Arc::new(
        SqliteCeremonyStore::open(path)
            .map_err(|error| ComposeError::CeremonyStore(format!("at {path}: {error}")))?,
    );
    let budgets = Arc::new(SqliteBudgetLedgerStore::open(path).map_err(|error| {
        ComposeError::CeremonyStore(format!("budget ledger at {path}: {error}"))
    })?);
    info!(path, "ceremony state is durable");

    let (memory_writer, memory_reader): (Arc<dyn MemoryWriterPort>, Arc<dyn MemoryReaderPort>) =
        if memory_selection == MemorySelection::None {
            info!("session memory is disabled explicitly with MADE_MEMORY=none");
            let memory = Arc::new(ForgetfulMemory::new());
            (memory.clone(), memory)
        } else {
            info!(
                path,
                "session memory is durable in the ceremony SQLite store"
            );
            (store.clone(), store.clone())
        };

    Ok(CeremonyPersistence {
        events: store.clone(),
        cursors: store.clone(),
        snapshots: store.clone(),
        publications: store.clone(),
        memory_writer,
        memory_reader,
        receipts: store,
        budgets,
    })
}

#[cfg(test)]
mod tests {
    use made_adapters::config::GrpcTlsConfig;

    use super::*;

    fn config(path: Option<String>, memory: MemorySelection) -> ServiceConfig {
        ServiceConfig {
            grpc_port: 50055,
            http_port: 8080,
            nats_enabled: false,
            nats_url: "nats://unused".to_owned(),
            trigger_subject: "made.trigger.>".to_owned(),
            publish_prefix: "made".to_owned(),
            postgres_url: None,
            ceremony_store_path: path,
            artifact_store_path: None,
            memory,
            grpc_tls: GrpcTlsConfig::Disabled,
            max_parallel: made_core::value_objects::MaxParallel::SERVER_MAX,
        }
    }

    #[test]
    fn a_path_enables_sqlite_memory_by_default() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("made.sqlite3");
        let wired = wire(
            &config(
                Some(path.to_string_lossy().into_owned()),
                MemorySelection::Automatic,
            ),
            None,
        )
        .unwrap();

        assert!(wired.memory_writer.capabilities().remembers());
        assert!(wired.memory_reader.capabilities().recalls());
    }

    #[test]
    fn explicit_sqlite_with_a_path_enables_durable_memory() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("made.sqlite3");
        let wired = wire(
            &config(
                Some(path.to_string_lossy().into_owned()),
                MemorySelection::Sqlite,
            ),
            None,
        )
        .unwrap();

        assert!(wired.memory_writer.capabilities().remembers());
    }

    #[test]
    fn explicit_none_disables_memory_even_when_state_is_durable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("made.sqlite3");
        let wired = wire(
            &config(
                Some(path.to_string_lossy().into_owned()),
                MemorySelection::None,
            ),
            None,
        )
        .unwrap();

        assert!(!wired.memory_writer.capabilities().remembers());
        assert!(!wired.memory_reader.capabilities().recalls());
    }

    #[test]
    fn automatic_without_a_path_is_the_explicit_forgetful_adapter() {
        let wired = wire(&config(None, MemorySelection::Automatic), None).unwrap();

        assert!(!wired.memory_writer.capabilities().remembers());
    }

    #[test]
    fn explicit_sqlite_without_a_path_is_a_configuration_error() {
        let error = wire(&config(None, MemorySelection::Sqlite), None)
            .err()
            .expect("sqlite without a store path must be refused");

        assert!(error.to_string().contains("MADE_CEREMONY_STORE_PATH"));
    }

    #[test]
    fn postgres_refuses_a_second_ceremony_store() {
        let config = config(Some("local.sqlite3".to_owned()), MemorySelection::Automatic);
        let error = validate_postgres_selection(&config).unwrap_err();

        assert!(error
            .to_string()
            .contains("select different ceremony stores"));
    }

    #[test]
    fn postgres_refuses_explicit_sqlite_memory() {
        let config = config(None, MemorySelection::Sqlite);
        let error = validate_postgres_selection(&config).unwrap_err();

        assert!(error.to_string().contains("MADE_MEMORY=sqlite"));
    }
}
