//! [`StartPublishedCeremonyUseCase`] — run a published definition, and

use std::sync::Arc;

use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::ports::{CeremonyDefinitionPublicationPort, ClockPort};

use super::start_ceremony_input::StartCeremonyInput;
use crate::services::{session_facts, SessionStream};

pub struct StartPublishedCeremonyUseCase {
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for StartPublishedCeremonyUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartPublishedCeremonyUseCase").finish()
    }
}

impl StartPublishedCeremonyUseCase {
    #[must_use]
    pub fn new(
        publications: Arc<dyn CeremonyDefinitionPublicationPort>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            publications,
            stream,
            clock,
        }
    }

    #[tracing::instrument(
        name = "start_published_ceremony",
        skip_all,
        fields(ceremony_id = %input.id)
    )]
    pub async fn execute(
        &self,
        input: StartCeremonyInput,
    ) -> Result<CeremonyInstance, DomainError> {
        // No `exists` check before storing. Asking and then storing
        // leaves a gap two concurrent starts both walk through, and the
        // second would replace the first in silence. The append itself
        // refuses, because it expects the stream to be empty.
        let published = self
            .publications
            .published(&input.definition_name, &input.definition_version)
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_ceremony_definition",
            })?;

        // Named before the opening is sealed so a caller who named
        // themselves badly is refused without a session being left
        // behind.
        let actor = session_facts::party(&input.actor_id, input.actor_kind)?;
        let now = self.clock.now();
        let started =
            CeremonyInstance::decide_start_bound(input.id, &published, input.context, now);
        self.stream
            .open(started, actor, now)
            .await
            .map(|session| session.instance)
    }
}
