//! Integrator bindings in the canonical embedded SQLite store.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{BindOutcome, BindReplacement, IntegratorBindingPort};
use made_core::value_objects::{
    IntegratorBinding, IntegratorBindingId, IntegratorScope, LoopProgressMark,
};
use time::OffsetDateTime;

use crate::delivery::StoredIntegratorBinding;
use crate::engine::{Engine, Key, ReadTx, Table};

use super::ceremony_store::{decode, encode};
use super::error::join_failure;
use super::SqliteCeremonyStore;

/// Who is driving each ceremony, durable across restarts.
#[derive(Debug, Clone)]
pub struct SqliteIntegratorBindings {
    engine: Arc<dyn Engine>,
}

impl SqliteIntegratorBindings {
    /// Open the bindings through the canonical store's lifecycle checks.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        SqliteCeremonyStore::open(path).map(|store| store.integrator_bindings())
    }

    pub(super) fn from_engine(engine: Arc<dyn Engine>) -> Self {
        Self { engine }
    }

    async fn blocking<T, F>(&self, op: &'static str, work: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(&dyn Engine) -> Result<T, DomainError> + Send + 'static,
    {
        let engine = Arc::clone(&self.engine);
        tokio::task::spawn_blocking(move || work(engine.as_ref()))
            .await
            .map_err(|error| join_failure(&error, op))?
    }
}

#[async_trait]
impl IntegratorBindingPort for SqliteIntegratorBindings {
    /// The incumbent is read and the scope written in one transaction,
    /// so two hosts cannot both believe they took over.
    async fn bind(
        &self,
        binding: IntegratorBinding,
        replacement: BindReplacement,
    ) -> Result<BindOutcome, DomainError> {
        self.blocking("bind integrator", move |engine| {
            let key = binding.scope_key().to_string();
            let mut tx = engine.begin_write()?;
            let stored = read(tx.as_ref(), &key)?;
            let (next, outcome) = stored.bind(binding, replacement);
            if let Some(next) = next {
                tx.insert(
                    Table::IntegratorBindings,
                    Key::Str(&key),
                    &encode(&next, "encode integrator binding")?,
                )?;
                tx.commit()?;
            }
            Ok(outcome)
        })
        .await
    }

    async fn current(
        &self,
        scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        let key = scope.scope_key().to_string();
        self.blocking("read integrator binding", move |engine| {
            let tx = engine.begin_read()?;
            Ok(read(tx.as_ref(), &key)?.current().cloned())
        })
        .await
    }

    /// A binding is revoked by its own identifier, which does not say
    /// which scope it belongs to, so the scopes are walked. There is one
    /// row per scope and a handful of scopes in flight, and the honest
    /// alternative — a second index nothing else reads — would be a
    /// second thing to keep true.
    async fn revoke(
        &self,
        id: &IntegratorBindingId,
        now: OffsetDateTime,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        let id = id.clone();
        self.blocking("revoke integrator binding", move |engine| {
            let mut tx = engine.begin_write()?;
            let found = tx
                .scan_str(Table::IntegratorBindings)?
                .into_iter()
                .map(|(key, value)| {
                    decode::<StoredIntegratorBinding>(&value, "decode integrator binding")
                        .map(|stored| (key, stored))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .find(|(_, stored)| stored.bindings().iter().any(|binding| binding.id() == &id));
            let Some((key, stored)) = found else {
                return Ok(None);
            };
            let (next, revoked) = stored.revoke(&id, now);
            if let Some(next) = next {
                tx.insert(
                    Table::IntegratorBindings,
                    Key::Str(&key),
                    &encode(&next, "encode integrator binding")?,
                )?;
                tx.commit()?;
            }
            Ok(revoked)
        })
        .await
    }

    /// Walked the same way a revocation is, and for the same reason:
    /// a binding identifier does not say which scope holds it.
    async fn record_progress(
        &self,
        id: &IntegratorBindingId,
        progress: LoopProgressMark,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        let id = id.clone();
        self.blocking("record integrator loop progress", move |engine| {
            let mut tx = engine.begin_write()?;
            let found = tx
                .scan_str(Table::IntegratorBindings)?
                .into_iter()
                .map(|(key, value)| {
                    decode::<StoredIntegratorBinding>(&value, "decode integrator binding")
                        .map(|stored| (key, stored))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .find(|(_, stored)| stored.bindings().iter().any(|binding| binding.id() == &id));
            let Some((key, stored)) = found else {
                return Ok(None);
            };
            let (next, observed) = stored.observing(&id, progress);
            if let Some(next) = next {
                tx.insert(
                    Table::IntegratorBindings,
                    Key::Str(&key),
                    &encode(&next, "encode integrator binding")?,
                )?;
                tx.commit()?;
            }
            Ok(observed)
        })
        .await
    }

    async fn list(
        &self,
        scope: Option<&IntegratorScope>,
    ) -> Result<Vec<IntegratorBinding>, DomainError> {
        let key = scope.map(|scope| scope.scope_key().to_string());
        self.blocking("list integrator bindings", move |engine| {
            let tx = engine.begin_read()?;
            if let Some(key) = key {
                return Ok(read(tx.as_ref(), &key)?.bindings().to_vec());
            }
            let mut every = Vec::new();
            for (_, value) in tx.scan_str(Table::IntegratorBindings)? {
                let stored: StoredIntegratorBinding = decode(&value, "decode integrator binding")?;
                every.extend(stored.bindings().to_vec());
            }
            Ok(every)
        })
        .await
    }
}

impl SqliteCeremonyStore {
    /// Integrator bindings sharing this store's open engine and pool.
    #[must_use]
    pub fn integrator_bindings(&self) -> SqliteIntegratorBindings {
        SqliteIntegratorBindings::from_engine(Arc::clone(&self.engine))
    }
}

fn read(tx: &dyn ReadTx, key: &str) -> Result<StoredIntegratorBinding, DomainError> {
    let stored = tx
        .get(Table::IntegratorBindings, Key::Str(key))?
        .map(|bytes| decode(&bytes, "decode integrator binding"))
        .transpose()?;
    Ok(stored.unwrap_or_else(StoredIntegratorBinding::new))
}
