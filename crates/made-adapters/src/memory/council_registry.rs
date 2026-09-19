//! In-memory [`CouncilRegistryPort`] backed by a `RwLock<BTreeMap>`.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::Council;
use made_core::error::DomainError;
use made_core::ports::CouncilRegistryPort;
use made_core::value_objects::{AuthorizationEvidence, Specialty};
use tokio::sync::RwLock;

type CouncilRegistryState = (BTreeMap<Specialty, Council>, Vec<AuthorizationEvidence>);

/// In-memory council registry keyed by [`Specialty`].
///
/// Cheap to `Clone`; internal state is shared through `Arc<RwLock>`.
#[derive(Debug, Default, Clone)]
pub struct InMemoryCouncilRegistry {
    inner: Arc<RwLock<CouncilRegistryState>>,
}

impl InMemoryCouncilRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of councils currently registered. Read-only helper for
    /// diagnostics and tests.
    pub async fn len(&self) -> usize {
        self.inner.read().await.0.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.0.is_empty()
    }
}

#[async_trait]
impl CouncilRegistryPort for InMemoryCouncilRegistry {
    async fn register(&self, council: Council) -> Result<(), DomainError> {
        self.register_authorized(council, None).await
    }

    async fn register_authorized(
        &self,
        council: Council,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        let mut map = self.inner.write().await;
        if map.0.contains_key(council.specialty()) {
            return Err(DomainError::AlreadyExists { what: "council" });
        }
        map.0.insert(council.specialty().clone(), council);
        map.1.extend(authorization);
        Ok(())
    }

    async fn replace(&self, council: Council) -> Result<(), DomainError> {
        self.replace_authorized(council, None).await
    }

    async fn replace_authorized(
        &self,
        council: Council,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        let mut map = self.inner.write().await;
        if !map.0.contains_key(council.specialty()) {
            return Err(DomainError::NotFound { what: "council" });
        }
        map.0.insert(council.specialty().clone(), council);
        map.1.extend(authorization);
        Ok(())
    }

    async fn get(&self, specialty: &Specialty) -> Result<Council, DomainError> {
        self.inner
            .read()
            .await
            .0
            .get(specialty)
            .cloned()
            .ok_or(DomainError::NotFound { what: "council" })
    }

    async fn list(&self) -> Result<Vec<Council>, DomainError> {
        Ok(self.inner.read().await.0.values().cloned().collect())
    }

    async fn delete(&self, specialty: &Specialty) -> Result<(), DomainError> {
        self.delete_authorized(specialty, None).await
    }

    async fn delete_authorized(
        &self,
        specialty: &Specialty,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        let mut state = self.inner.write().await;
        state
            .0
            .remove(specialty)
            .ok_or(DomainError::NotFound { what: "council" })?;
        state.1.extend(authorization);
        Ok(())
    }

    async fn contains(&self, specialty: &Specialty) -> Result<bool, DomainError> {
        Ok(self.inner.read().await.0.contains_key(specialty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::{AgentId, CouncilId};
    use serde_json::json;
    use time::macros::datetime;

    fn council(specialty: &str) -> Council {
        Council::new(
            CouncilId::new(specialty).unwrap(),
            Specialty::new(specialty).unwrap(),
            vec![AgentId::new("a").unwrap()],
            datetime!(2026-04-15 12:00:00 UTC),
        )
        .unwrap()
    }

    fn evidence(request_id: &str) -> AuthorizationEvidence {
        serde_json::from_value(json!({
            "decision_id": "a".repeat(64),
            "request_id": request_id,
            "principal_id": "council-owner",
            "action": "create_council",
            "scope": {"kind": "global"},
            "target_digest": "b".repeat(64),
            "policy_version": 1,
            "admitted_at": "2026-09-19T12:00:00Z",
            "valid_until": "2026-09-19T12:01:00Z"
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn register_then_get_roundtrips() {
        let reg = InMemoryCouncilRegistry::new();
        reg.register(council("triage")).await.unwrap();
        let got = reg.get(&Specialty::new("triage").unwrap()).await.unwrap();
        assert_eq!(got.specialty().as_str(), "triage");
    }

    #[tokio::test]
    async fn duplicate_register_rejected() {
        let reg = InMemoryCouncilRegistry::new();
        reg.register(council("x")).await.unwrap();
        let err = reg.register(council("x")).await.unwrap_err();
        assert!(matches!(err, DomainError::AlreadyExists { .. }));
    }

    #[tokio::test]
    async fn authorized_mutation_retains_exact_evidence_only_when_it_lands() {
        let reg = InMemoryCouncilRegistry::new();
        let accepted = evidence("accepted-council-request");
        reg.register_authorized(council("x"), Some(accepted.clone()))
            .await
            .unwrap();

        let rejected = evidence("rejected-council-request");
        assert!(matches!(
            reg.register_authorized(council("x"), Some(rejected)).await,
            Err(DomainError::AlreadyExists { what: "council" })
        ));

        assert_eq!(reg.inner.read().await.1, vec![accepted]);
    }

    #[tokio::test]
    async fn replace_requires_existing_council() {
        let reg = InMemoryCouncilRegistry::new();
        let err = reg.replace(council("x")).await.unwrap_err();
        assert!(matches!(err, DomainError::NotFound { .. }));

        reg.register(council("x")).await.unwrap();
        reg.replace(council("x")).await.unwrap();
    }

    #[tokio::test]
    async fn list_reports_everything() {
        let reg = InMemoryCouncilRegistry::new();
        reg.register(council("a")).await.unwrap();
        reg.register(council("b")).await.unwrap();
        let all = reg.list().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn delete_removes_entry() {
        let reg = InMemoryCouncilRegistry::new();
        reg.register(council("x")).await.unwrap();
        reg.delete(&Specialty::new("x").unwrap()).await.unwrap();
        assert!(reg.is_empty().await);

        let err = reg.delete(&Specialty::new("x").unwrap()).await.unwrap_err();
        assert!(matches!(err, DomainError::NotFound { .. }));
    }

    #[tokio::test]
    async fn contains_reflects_state() {
        let reg = InMemoryCouncilRegistry::new();
        let sp = Specialty::new("x").unwrap();
        assert!(!reg.contains(&sp).await.unwrap());
        reg.register(council("x")).await.unwrap();
        assert!(reg.contains(&sp).await.unwrap());
    }

    #[tokio::test]
    async fn clone_shares_state() {
        let a = InMemoryCouncilRegistry::new();
        let b = a.clone();
        a.register(council("x")).await.unwrap();
        assert_eq!(b.len().await, 1);
    }
}
