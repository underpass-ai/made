use std::sync::Arc;

use made_core::entities::ceremony_commands::EnforceCeremonyDeadlines;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;
use made_core::value_objects::AuditActorKind;

use super::{EnforceCeremonyDeadlinesInput, ResolveCeremonyDefinitionUseCase};
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct EnforceCeremonyDeadlinesUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for EnforceCeremonyDeadlinesUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EnforceCeremonyDeadlinesUseCase")
            .finish()
    }
}

impl EnforceCeremonyDeadlinesUseCase {
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
        input: EnforceCeremonyDeadlinesInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::party("made.lifecycle.deadline", AuditActorKind::Engine)?;
        let now = self.clock.now();
        let command = CeremonyCommand::EnforceCeremonyDeadlines(EnforceCeremonyDeadlines { now });
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
