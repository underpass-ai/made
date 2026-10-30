//! [`StartPublishedProtocolUseCase`] — run a published definition, and

use std::sync::Arc;

use made_core::entities::ProtocolInstance;
use made_core::error::DomainError;
use made_core::ports::{ClockPort, MemoryReaderPort, ProtocolDefinitionPublicationPort};

use super::start_protocol_input::StartProtocolInput;
use crate::services::{memory_scope_resolver, session_facts, session_recall, SessionStream};

pub struct StartPublishedProtocolUseCase {
    publications: Arc<dyn ProtocolDefinitionPublicationPort>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
    memory: Arc<dyn MemoryReaderPort>,
}

impl std::fmt::Debug for StartPublishedProtocolUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartPublishedProtocolUseCase").finish()
    }
}

impl StartPublishedProtocolUseCase {
    #[must_use]
    pub fn new(
        publications: Arc<dyn ProtocolDefinitionPublicationPort>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
        memory: Arc<dyn MemoryReaderPort>,
    ) -> Self {
        Self {
            publications,
            stream,
            clock,
            memory,
        }
    }

    #[tracing::instrument(
        name = "start_published_protocol",
        skip_all,
        fields(protocol_id = %input.id)
    )]
    pub async fn execute(
        &self,
        input: StartProtocolInput,
    ) -> Result<ProtocolInstance, DomainError> {
        // No `exists` check before storing. Asking and then storing
        // leaves a gap two concurrent starts both walk through, and the
        // second would replace the first in silence. The append itself
        // refuses, because it expects the stream to be empty.
        let published = self
            .publications
            .published(&input.definition_name, &input.definition_version)
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_protocol_definition",
            })?;

        // Named before the opening is sealed so a caller who named
        // themselves badly is refused without a session being left
        // behind.
        let actor = session_facts::party(input.actor_id.as_str(), input.actor_kind)?;
        let scope = memory_scope_resolver::of_context(&input.context, &input.id)?;
        let recalled = session_recall::recall(self.memory.as_ref(), &scope).await;
        let now = self.clock.now();
        let opening = ProtocolInstance::decide_start_bound(
            input.id,
            &published,
            input.context,
            recalled,
            now,
        )?;
        self.stream
            .open(opening, actor, now)
            .await
            .map(|session| session.instance)
    }
}

#[cfg(test)]
mod tests {
    use made_core::ports::MemoryWriterPort;
    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, MemoryScope, MemoryWrite, ProtocolContext,
    };

    use super::*;
    use crate::usecases::protocol_test_support::{
        definition, definition_name, now, protocol_id, recording_memory, remembered_decision,
        stream_over, version, EventStoreFake, FixedClock, PublicationsFake,
    };

    /// A published definition is started the same way an unpublished
    /// one is, so it is told the same thing. Proven here rather than
    /// assumed from the other use case: they are two code paths, and
    /// the last time one of them grew a field the other did not, that
    /// was a parity defect with a name (F2).
    #[tokio::test]
    async fn a_published_session_is_told_what_its_scope_holds() {
        let publications = Arc::new(PublicationsFake::default());
        publications.seed(definition()).await;
        let memory = recording_memory();
        memory
            .remember(
                &MemoryScope::new("team:alpha").unwrap(),
                MemoryWrite::unexplained(vec![remembered_decision("the reviewer signs off first")])
                    .unwrap(),
                "seed",
            )
            .await
            .unwrap();
        let (stream, store) = stream_over(Arc::new(EventStoreFake::default()));
        let usecase = StartPublishedProtocolUseCase::new(
            publications,
            stream,
            Arc::new(FixedClock::new(now())),
            memory,
        );

        let instance = usecase
            .execute(StartProtocolInput::new(
                protocol_id(),
                definition_name(),
                version(),
                ProtocolContext::new(
                    Attributes::new(
                        [("memory_scope".to_owned(), serde_json::json!("team:alpha"))]
                            .into_iter()
                            .collect(),
                    )
                    .unwrap(),
                ),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        assert_eq!(
            instance
                .recollection()
                .expect("the session was told what the scope holds")
                .entries()[0]
                .summary(),
            "the reviewer signs off first"
        );
        let facts = store.facts().await;
        assert_eq!(facts.len(), 2, "{facts:?}");
        assert_eq!(facts[1].event.event_type(), AuditEventType::MemoryRecalled);
        assert!(
            instance.is_bound_to_a_published_definition(),
            "the binding survived the second fact"
        );
    }
}
