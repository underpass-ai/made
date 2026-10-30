use std::sync::Arc;

use made_core::entities::protocol_commands::CancelProtocol;
use made_core::entities::{ProtocolCommand, ProtocolInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::{CancelProtocolInput, ResolveProtocolDefinitionUseCase};
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct CancelProtocolUseCase {
    definitions: Arc<ResolveProtocolDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for CancelProtocolUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CancelProtocolUseCase").finish()
    }
}

impl CancelProtocolUseCase {
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
        input: CancelProtocolInput,
    ) -> Result<ProtocolInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::party(input.actor_id.as_str(), input.actor_kind)?;
        let now = self.clock.now();
        let command = ProtocolCommand::CancelProtocol(CancelProtocol {
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
