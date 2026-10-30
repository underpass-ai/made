use std::sync::Arc;

use made_core::entities::protocol_commands::ResumeProtocol;
use made_core::entities::{ProtocolCommand, ProtocolInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::{ResolveProtocolDefinitionUseCase, ResumeProtocolInput};
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct ResumeProtocolUseCase {
    definitions: Arc<ResolveProtocolDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for ResumeProtocolUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ResumeProtocolUseCase").finish()
    }
}

impl ResumeProtocolUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveProtocolDefinitionUseCase>,
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
        input: ResumeProtocolInput,
    ) -> Result<ProtocolInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::party(input.actor_id.as_str(), input.actor_kind)?;
        let now = self.clock.now();
        let command = ProtocolCommand::ResumeProtocol(ResumeProtocol { now });
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
