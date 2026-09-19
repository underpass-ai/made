use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;
use made_core::ports::{MemoryReaderPort, MemoryRecollection};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationRequest, AuthorizationRequestId, AuthorizationScope,
    AuthorizationTargetDigest, AuthorizedOperation, CeremonyId, MemoryCapabilities, MemoryEntry,
    MemoryEntryId, MemoryMoment, MemoryScope,
};
use made_core::DomainError;
use serde_json::{json, Value};

use super::{AuthorizationGateOutcome, AuthorizeOperationUseCase};
use crate::services::{AuthorizationOperationScope, SessionStream};

/// A shared memory scope selects candidates; authority comes from each source ceremony.
pub struct AuthorizedMemoryReader {
    inner: Arc<dyn MemoryReaderPort>,
    authorize: Arc<AuthorizeOperationUseCase>,
    sources: Arc<SessionStream>,
}

impl std::fmt::Debug for AuthorizedMemoryReader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthorizedMemoryReader")
            .finish_non_exhaustive()
    }
}

impl AuthorizedMemoryReader {
    #[must_use]
    pub fn new(
        inner: Arc<dyn MemoryReaderPort>,
        authorize: Arc<AuthorizeOperationUseCase>,
        sources: Arc<SessionStream>,
    ) -> Self {
        Self {
            inner,
            authorize,
            sources,
        }
    }

    fn operation() -> Result<AuthorizedOperation, DomainError> {
        AuthorizationOperationScope::current().ok_or(DomainError::InvariantViolated {
            reason: "protected memory read requires an authenticated operation",
        })
    }

    async fn allowed(
        &self,
        source: &CeremonyId,
        operation: &AuthorizedOperation,
        query: &Value,
    ) -> Result<bool, DomainError> {
        let scope = match self.sources.load(source).await {
            Ok(session) => AuthorizationScope::ResolvedCeremony {
                ceremony_id: source.clone(),
                root_id: session
                    .instance
                    .lineage()
                    .map_or_else(|| source.clone(), |lineage| lineage.root_id().clone()),
            },
            // External memory can name a source absent from this store. It still
            // requires explicit authority for that exact id; no lineage is guessed.
            Err(DomainError::NotFound { .. }) => AuthorizationScope::Ceremony {
                ceremony_id: source.clone(),
            },
            Err(error) => return Err(error),
        };
        let target = serde_json::to_vec(&json!({
            "operation":"memory_source_read_v1", "query":query, "source":source,
        }))
        .map_err(|_| DomainError::InvariantViolated {
            reason: "memory read target cannot be serialized",
        })?;
        let target_digest = AuthorizationTargetDigest::for_bytes(&target);
        let identity = serde_json::to_vec(&(
            "memory_source_read_v1",
            operation.evidence().request_id(),
            &target_digest,
        ))
        .map_err(|_| DomainError::InvariantViolated {
            reason: "memory read identity cannot be serialized",
        })?;
        let request = AuthorizationRequest::new(
            AuthorizationRequestId::new(format!(
                "memory-{}",
                AuthorizationTargetDigest::for_bytes(&identity).as_str()
            ))?,
            operation.principal().clone(),
            AuthorizationAction::ReadCeremonyEvents,
            scope,
            target_digest,
        );
        Ok(matches!(
            self.authorize.execute(request).await?,
            AuthorizationGateOutcome::Allowed { .. }
        ))
    }

    async fn filter(
        &self,
        recalled: MemoryRecollection,
        operation: &AuthorizedOperation,
        query: &Value,
    ) -> Result<MemoryRecollection, DomainError> {
        let MemoryRecollection::Recalled { entries, relations } = recalled else {
            return Ok(MemoryRecollection::Unsupported);
        };
        // Backends can merge independently named memories. A colliding id from
        // different sources has no unambiguous provenance, so neither its
        // content nor relations attached to the entry are safe to disclose.
        let mut origins = BTreeMap::new();
        let mut ambiguous = BTreeSet::new();
        for entry in &entries {
            if let Some(previous) = origins.insert(entry.id(), entry.provenance().ceremony_id()) {
                if previous != entry.provenance().ceremony_id() {
                    ambiguous.insert(entry.id().clone());
                }
            }
        }
        let mut permissions = BTreeMap::new();
        let mut permitted = Vec::new();
        let mut permitted_ids = BTreeSet::new();
        for entry in entries {
            if ambiguous.contains(entry.id()) {
                continue;
            }
            let source = entry.provenance().ceremony_id();
            let allowed = if let Some(allowed) = permissions.get(source) {
                *allowed
            } else {
                let allowed = self.allowed(source, operation, query).await?;
                permissions.insert(source.clone(), allowed);
                allowed
            };
            if allowed {
                permitted_ids.insert(entry.id().clone());
                permitted.push(entry);
            }
        }
        Ok(MemoryRecollection::Recalled {
            entries: permitted,
            relations: relations
                .into_iter()
                .filter(|relation| {
                    permitted_ids.contains(relation.from()) && permitted_ids.contains(relation.to())
                })
                .collect(),
        })
    }
}

#[async_trait]
impl MemoryReaderPort for AuthorizedMemoryReader {
    async fn recall(&self, scope: &MemoryScope) -> Result<MemoryRecollection, DomainError> {
        let operation = Self::operation()?;
        let recalled = self.inner.recall(scope).await?;
        self.filter(
            recalled,
            &operation,
            &json!({"kind":"recall","scope":scope}),
        )
        .await
    }

    async fn as_known_at(
        &self,
        scope: &MemoryScope,
        moment: MemoryMoment,
    ) -> Result<MemoryRecollection, DomainError> {
        let operation = Self::operation()?;
        let recalled = self.inner.as_known_at(scope, moment).await?;
        self.filter(
            recalled,
            &operation,
            &json!({"kind":"as_known_at","scope":scope,"moment":moment}),
        )
        .await
    }

    async fn follow(
        &self,
        scope: &MemoryScope,
        from: &MemoryEntryId,
        to: &MemoryEntryId,
    ) -> Result<MemoryRecollection, DomainError> {
        let operation = Self::operation()?;
        let path = self.inner.follow(scope, from, to).await?;
        if !path.is_supported() {
            return Ok(path);
        }
        // A follow answer deliberately has no entries. Resolve the endpoint
        // provenance privately, then return only edges between readable entries.
        let candidates = self.inner.recall(scope).await?;
        let visible = self
            .filter(
                candidates,
                &operation,
                &json!({"kind":"follow","scope":scope,"from":from,"to":to}),
            )
            .await?;
        let permitted: BTreeSet<_> = visible.entries().iter().map(MemoryEntry::id).collect();
        // A partial chain also reveals that a route through an unreadable
        // endpoint exists. Disclose a followed path only when the complete
        // answer, including both requested endpoints, is authorized.
        if !permitted.contains(from)
            || !permitted.contains(to)
            || path.relations().iter().any(|relation| {
                !permitted.contains(relation.from()) || !permitted.contains(relation.to())
            })
        {
            return Ok(MemoryRecollection::nothing());
        }
        Ok(path)
    }

    fn capabilities(&self) -> MemoryCapabilities {
        self.inner.capabilities()
    }
}
