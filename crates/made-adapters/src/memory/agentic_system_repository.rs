use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::AgenticSystem;
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemPage, AgenticSystemQuery, AgenticSystemRepositoryPort, AgenticSystemSaveOutcome,
};
use made_core::value_objects::{AgenticSystemId, AgenticSystemRevision};
use tokio::sync::RwLock;

/// Process-local designs, every revision kept in order.
///
/// A vector per design rather than a head plus history: the revision
/// log *is* the storage, and a store that kept only the head would
/// pass the conflict test and fail the one that reads back what a
/// design said before somebody changed it.
#[derive(Debug, Default, Clone)]
pub struct InMemoryAgenticSystemRepository {
    inner: Arc<RwLock<BTreeMap<AgenticSystemId, Vec<AgenticSystem>>>>,
}

impl InMemoryAgenticSystemRepository {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AgenticSystemRepositoryPort for InMemoryAgenticSystemRepository {
    async fn save(
        &self,
        system: AgenticSystem,
        expected: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemSaveOutcome, DomainError> {
        let mut designs = self.inner.write().await;
        // Looked up, not `entry`-ed: a refused save must leave no
        // trace, not even an empty history for an id nobody stored.
        let head = designs
            .get(system.id())
            .and_then(|history| history.last())
            .map(AgenticSystem::revision);
        match (head, expected) {
            (None, Some(_)) => return Ok(AgenticSystemSaveOutcome::absent()),
            (Some(current), _) if head != expected => {
                return Ok(AgenticSystemSaveOutcome::conflict(current));
            }
            _ => {}
        }
        let revision = head.map_or(AgenticSystemRevision::INITIAL, AgenticSystemRevision::next);
        designs
            .entry(system.id().clone())
            .or_default()
            .push(system.at_revision(revision));
        Ok(AgenticSystemSaveOutcome::saved(revision))
    }

    async fn get(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
    ) -> Result<Option<AgenticSystem>, DomainError> {
        let designs = self.inner.read().await;
        let Some(history) = designs.get(id) else {
            return Ok(None);
        };
        Ok(match revision {
            Some(revision) => history
                .iter()
                .find(|system| system.revision() == revision)
                .cloned(),
            None => history.last().cloned(),
        })
    }

    async fn list(&self, query: &AgenticSystemQuery) -> Result<AgenticSystemPage, DomainError> {
        let designs = self.inner.read().await;
        let mut admitted: Vec<AgenticSystem> = designs
            .values()
            .filter_map(|history| history.last())
            .filter(|system| query.admits(system.id(), system.lifecycle()))
            .cloned()
            .collect();
        let next_cursor = (admitted.len() > query.limit().as_usize())
            .then(|| admitted[query.limit().as_usize() - 1].id().clone());
        admitted.truncate(query.limit().as_usize());
        Ok(AgenticSystemPage::new(admitted, next_cursor))
    }
}

#[cfg(test)]
mod tests {
    use made_core::value_objects::{
        AttentionPolicy, CeremonyActivation, CeremonyComposition, CeremonyDefinitionDigest,
        CeremonyName, CeremonyVersion, DefinitionPin, LogicalParticipant, ParticipantBindingPolicy,
        ParticipantId, ParticipantKind, Responsibility, SupervisionPolicy, SystemCeremonyId,
        SystemPurpose, SystemRole, SystemRoleId, SystemRoleKind,
    };
    use time::OffsetDateTime;

    use super::*;

    /// Not observable through the port — `get` and `list` skip empty
    /// histories — which is why it is pinned here: the durable stores
    /// write nothing on a refusal, and this one must not either.
    #[tokio::test]
    async fn a_refused_save_records_nothing() {
        let repository = InMemoryAgenticSystemRepository::new();

        let outcome = repository
            .save(design(), Some(AgenticSystemRevision::INITIAL))
            .await
            .unwrap();

        assert_eq!(outcome, AgenticSystemSaveOutcome::absent());
        assert!(repository.inner.read().await.is_empty());
    }

    fn design() -> AgenticSystem {
        let integrator = SystemRoleId::new("integrator").unwrap();
        AgenticSystem::draft(
            AgenticSystemId::new("refused").unwrap(),
            SystemPurpose::new("an edit of nothing").unwrap(),
            integrator.clone(),
            [SystemRole::new(
                integrator.clone(),
                Responsibility::new("drives the system").unwrap(),
                SystemRoleKind::Integrator,
            )],
            [LogicalParticipant::new(
                ParticipantId::new("operator").unwrap(),
                integrator,
                ParticipantKind::Person,
                ParticipantBindingPolicy::default(),
            )],
            [],
            [],
            [CeremonyComposition::new(
                SystemCeremonyId::new("delivery").unwrap(),
                DefinitionPin::new(
                    CeremonyName::new("delivery").unwrap(),
                    CeremonyVersion::new("1.0").unwrap(),
                    CeremonyDefinitionDigest::from_bytes([0x0a; 32]),
                ),
                SystemPurpose::new("does the work").unwrap(),
                [],
                CeremonyActivation::Manual,
                [],
                [],
            )],
            SupervisionPolicy::default(),
            AttentionPolicy::default(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
    }
}
