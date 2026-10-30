use std::sync::Arc;

use made_core::entities::protocol_commands::PauseProtocol;
use made_core::entities::{ProtocolCommand, ProtocolInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::{PauseProtocolInput, ResolveProtocolDefinitionUseCase};
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct PauseProtocolUseCase {
    definitions: Arc<ResolveProtocolDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for PauseProtocolUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("PauseProtocolUseCase").finish()
    }
}

impl PauseProtocolUseCase {
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
        input: PauseProtocolInput,
    ) -> Result<ProtocolInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::party(input.actor_id.as_str(), input.actor_kind)?;
        let now = self.clock.now();
        let command = ProtocolCommand::PauseProtocol(PauseProtocol {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usecases::protocol_test_support::{
        definition, definition_resolver, now, protocol_id, started_instance,
        stream_conflicting_once, EventStoreFake, FixedClock,
    };
    use made_core::value_objects::{AuditActorKind, AuditEventType, LifecycleReason};

    #[tokio::test]
    async fn retries_a_pause_race_and_seals_one_pause() {
        let definition = definition();
        let definitions = Arc::new(
            crate::usecases::protocol_test_support::DefinitionRepositoryFake::new(
                definition.clone(),
            ),
        );
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();
        let usecase = PauseProtocolUseCase::new(
            definition_resolver(definitions),
            stream_conflicting_once(store.clone()),
            Arc::new(FixedClock::new(now())),
        );

        let paused = usecase
            .execute(PauseProtocolInput::new(
                protocol_id(),
                "operator-1",
                AuditActorKind::Human,
                LifecycleReason::new("maintenance").unwrap(),
            ))
            .await
            .unwrap();

        assert!(paused.is_paused());
        assert_eq!(
            store
                .records(&protocol_id())
                .await
                .iter()
                .filter(|record| record.event_type() == AuditEventType::ProtocolPaused)
                .count(),
            1
        );
    }
}
