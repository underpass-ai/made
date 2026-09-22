//! [`SessionDefinitionLookup`] — the ordinary way to resolve a
//! definition, folded from the stream it was started on.

use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::CeremonyDefinition;
use made_core::error::DomainError;
use made_core::ports::{CeremonyDefinitionPublicationPort, CeremonyDefinitionRepositoryPort};
use made_core::ports::{CeremonyEventStorePort, CeremonySnapshotStorePort};
use made_core::value_objects::CeremonyId;

use crate::services::{CeremonyEventFanout, SessionStream};
use crate::usecases::ResolveCeremonyDefinitionUseCase;

use super::CeremonyDefinitionLookup;

/// Folds a session far enough to learn which definition it runs, and
/// resolves that.
///
/// It keeps a stream of its own, told to fan out to nobody. The
/// engine's stream is built after the projection that needs this, and
/// this one only ever reads — a second reader of the same two stores,
/// not a second writer of anything.
pub struct SessionDefinitionLookup {
    stream: SessionStream,
    definitions: ResolveCeremonyDefinitionUseCase,
}

impl std::fmt::Debug for SessionDefinitionLookup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SessionDefinitionLookup")
            .finish_non_exhaustive()
    }
}

impl SessionDefinitionLookup {
    #[must_use]
    pub fn over(
        events: Arc<dyn CeremonyEventStorePort>,
        snapshots: Arc<dyn CeremonySnapshotStorePort>,
        definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
        publications: Arc<dyn CeremonyDefinitionPublicationPort>,
    ) -> Self {
        Self {
            stream: SessionStream::new(
                events,
                snapshots,
                Arc::new(CeremonyEventFanout::new(Vec::new())),
            ),
            definitions: ResolveCeremonyDefinitionUseCase::new(definitions, publications),
        }
    }
}

#[async_trait]
impl CeremonyDefinitionLookup for SessionDefinitionLookup {
    /// The definition, or nothing at all.
    ///
    /// A session that will not load and a definition that will not
    /// resolve are the same answer here. The alternative is a
    /// projection round that fails over a reading it might not even
    /// have produced, which would stop the loop for every other
    /// ceremony in the deployment.
    async fn definition_of(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<Option<CeremonyDefinition>, DomainError> {
        let Ok(session) = self.stream.load(ceremony_id).await else {
            return Ok(None);
        };
        Ok(self.definitions.execute(&session.instance).await.ok())
    }
}
