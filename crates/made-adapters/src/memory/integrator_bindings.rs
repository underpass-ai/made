use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{BindOutcome, BindReplacement, IntegratorBindingPort};
use made_core::value_objects::{
    IntegratorBinding, IntegratorBindingId, IntegratorScope, IntegratorScopeKey, LoopProgressMark,
};
use time::OffsetDateTime;
use tokio::sync::RwLock;

use crate::delivery::StoredIntegratorBinding;

/// Process-local integrator bindings, one row per scope.
#[derive(Debug, Default, Clone)]
pub struct InMemoryIntegratorBindings {
    inner: Arc<RwLock<BTreeMap<IntegratorScopeKey, StoredIntegratorBinding>>>,
}

impl InMemoryIntegratorBindings {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl IntegratorBindingPort for InMemoryIntegratorBindings {
    async fn bind(
        &self,
        binding: IntegratorBinding,
        replacement: BindReplacement,
    ) -> Result<BindOutcome, DomainError> {
        let key = binding.scope_key();
        let mut scopes = self.inner.write().await;
        let stored = scopes
            .get(&key)
            .cloned()
            .unwrap_or_else(StoredIntegratorBinding::new);
        let (next, outcome) = stored.bind(binding, replacement);
        if let Some(next) = next {
            scopes.insert(key, next);
        }
        Ok(outcome)
    }

    async fn current(
        &self,
        scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        Ok(self
            .inner
            .read()
            .await
            .get(&scope.scope_key())
            .and_then(|stored| stored.current().cloned()))
    }

    async fn revoke(
        &self,
        id: &IntegratorBindingId,
        now: OffsetDateTime,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        let mut scopes = self.inner.write().await;
        let Some((key, stored)) = scopes
            .iter()
            .find(|(_, stored)| stored.bindings().iter().any(|binding| binding.id() == id))
            .map(|(key, stored)| (key.clone(), stored.clone()))
        else {
            return Ok(None);
        };
        let (next, revoked) = stored.revoke(id, now);
        if let Some(next) = next {
            scopes.insert(key, next);
        }
        Ok(revoked)
    }

    /// Straight to the scope's own row: the binding carries it.
    async fn record_progress(
        &self,
        binding: &IntegratorBinding,
        progress: LoopProgressMark,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        let key = binding.scope_key();
        let mut scopes = self.inner.write().await;
        let Some(stored) = scopes.get(&key).cloned() else {
            return Ok(None);
        };
        let (next, observed) = stored.observing(binding.id(), progress);
        if let Some(next) = next {
            scopes.insert(key, next);
        }
        Ok(observed)
    }

    async fn list(
        &self,
        scope: Option<&IntegratorScope>,
    ) -> Result<Vec<IntegratorBinding>, DomainError> {
        let scopes = self.inner.read().await;
        let Some(scope) = scope else {
            return Ok(scopes
                .values()
                .flat_map(|stored| stored.bindings().to_vec())
                .collect());
        };
        Ok(scopes
            .get(&scope.scope_key())
            .map(|stored| stored.bindings().to_vec())
            .unwrap_or_default())
    }
}
