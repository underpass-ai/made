use std::sync::Arc;

use made_core::entities::ceremony_commands::CancelCeremony;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::{CancelCeremonyInput, ResolveCeremonyDefinitionUseCase};
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct CancelCeremonyUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for CancelCeremonyUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CancelCeremonyUseCase").finish()
    }
}

impl CancelCeremonyUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            stream,
            clock,
        }
    }

    pub async fn execute(
        &self,
        input: CancelCeremonyInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::party(&input.actor_id, input.actor_kind)?;
        let now = self.clock.now();
        let command = CeremonyCommand::CancelCeremony(CancelCeremony {
            reason: input.reason,
            now,
        });
        Ok(self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&command, &definition)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await?
            .instance)
    }
}
