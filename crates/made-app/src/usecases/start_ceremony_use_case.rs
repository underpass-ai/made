//! [`StartCeremonyUseCase`] — create a ceremony instance from a definition.

use std::sync::Arc;

use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::ports::{CeremonyDefinitionRepositoryPort, ClockPort, MemoryReaderPort};

use super::start_ceremony_input::StartCeremonyInput;
use crate::services::{memory_scope_resolver, session_facts, session_recall, SessionStream};

pub struct StartCeremonyUseCase {
    definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
    memory: Arc<dyn MemoryReaderPort>,
}

impl std::fmt::Debug for StartCeremonyUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartCeremonyUseCase").finish()
    }
}

impl StartCeremonyUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
        memory: Arc<dyn MemoryReaderPort>,
    ) -> Self {
        Self {
            definitions,
            stream,
            clock,
            memory,
        }
    }

    #[tracing::instrument(
        name = "start_ceremony",
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
        let definition = self
            .definitions
            .get(&input.definition_name, &input.definition_version)
            .await?;
        // Named before the opening is sealed so a caller who named
        // themselves badly is refused without a session being left
        // behind.
        let actor = session_facts::party(&input.actor_id, input.actor_kind)?;
        // Resolved before anything is sealed: a scope the caller
        // declared and got wrong is a caller's mistake, and refusing it
        // here is what keeps it from becoming a session that quietly
        // remembers alone.
        let scope = memory_scope_resolver::of_context(&input.context, &input.id)?;
        let recalled = session_recall::recall(self.memory.as_ref(), &scope).await;
        let now = self.clock.now();
        let opening =
            CeremonyInstance::decide_start(input.id, &definition, input.context, recalled, now)?;
        self.stream
            .open(opening, actor, now)
            .await
            .map(|session| session.instance)
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::CeremonyEvent;
    use std::sync::Arc;

    use made_core::error::DomainError;
    use made_core::ports::MemoryWriterPort;
    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, CeremonyContext, MemoryScope, MemoryWrite,
        StateId,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        a_memory, ceremony_id, definition, definition_name, now, recording_memory,
        remembered_decision, started_instance, stream, stream_over, version,
        DefinitionRepositoryFake, EventStoreFake, FixedClock, MemoryThatIsOut,
    };

    #[tokio::test]
    async fn starts_and_persists_instance_at_initial_state() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition));
        let instances = Arc::new(EventStoreFake::default());
        let usecase = StartCeremonyUseCase::new(
            definitions,
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
            a_memory(),
        );

        let instance = usecase
            .execute(StartCeremonyInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                CeremonyContext::empty(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        assert_eq!(
            instance.current_state(),
            &StateId::new("COLLECTING_VOICES").unwrap()
        );
        let saved = instances.saved(&ceremony_id()).await;
        assert_eq!(saved.id(), &ceremony_id());
    }

    #[tokio::test]
    async fn duplicate_instance_id_is_rejected_before_overwrite() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let usecase = StartCeremonyUseCase::new(
            definitions,
            stream(instances),
            Arc::new(FixedClock::new(now())),
            a_memory(),
        );

        let err = usecase
            .execute(StartCeremonyInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                CeremonyContext::empty(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            DomainError::AlreadyExists {
                what: "ceremony_instance"
            }
        ));
    }

    /// Opening a session is the journal's first entry.
    ///
    /// The actor has no seat, on purpose: at the start the definition's
    /// roles are not filled, and whoever opened this may never take
    /// part in it.
    #[tokio::test]
    async fn seals_the_opening_into_the_journal() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let (stream, store) = stream_over(instances);
        let usecase = StartCeremonyUseCase::new(
            definitions,
            stream,
            Arc::new(FixedClock::new(now())),
            a_memory(),
        );

        let instance = usecase
            .execute(StartCeremonyInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                CeremonyContext::empty(),
                "scheduler-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        let facts = store.facts().await;
        assert_eq!(facts.len(), 1, "one opening, one fact: {facts:?}");
        assert_eq!(
            facts[0].event.event_type(),
            AuditEventType::CeremonyInstanceStarted
        );
        let CeremonyEvent::CeremonyInstanceStarted(started) = &facts[0].event else {
            panic!("an opening seals what it opened: {:?}", facts[0].event);
        };
        assert_eq!(started.initial_state, *instance.current_state());
        assert_eq!(
            started.step_ids,
            instance.step_records().keys().cloned().collect()
        );
        assert_eq!(started.bound_definition, None);
        assert_eq!(facts[0].actor.kind(), AuditActorKind::Service);
        assert_eq!(facts[0].actor.actor_id(), "scheduler-1");
        assert!(
            facts[0].actor.role_id().is_none(),
            "the opener was given a seat this ceremony never assigned"
        );
    }

    /// A session that declares no scope recalls nothing, and its stream
    /// is the stream it would have had before memory could be read at
    /// all: one fact, the opening.
    #[tokio::test]
    async fn a_session_that_declares_no_scope_seals_only_its_opening() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let memory = recording_memory();
        memory
            .remember(
                &MemoryScope::new("team:alpha").unwrap(),
                MemoryWrite::unexplained(vec![remembered_decision(
                    "roll back rather than restart",
                )])
                .unwrap(),
                "seed",
            )
            .await
            .unwrap();
        let (stream, store) = stream_over(Arc::new(EventStoreFake::default()));
        let usecase = StartCeremonyUseCase::new(
            definitions,
            stream,
            Arc::new(FixedClock::new(now())),
            memory,
        );

        usecase
            .execute(StartCeremonyInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                CeremonyContext::empty(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        let facts = store.facts().await;
        assert_eq!(
            facts.len(),
            1,
            "a session with nothing to recall sealed something: {facts:?}"
        );
        assert_eq!(
            facts[0].event.event_type(),
            AuditEventType::CeremonyInstanceStarted
        );
    }

    /// The whole of E1 in one test: what one session decided is what
    /// the next session in that scope is told when it opens.
    #[tokio::test]
    async fn a_declared_scope_brings_what_it_holds_into_the_opening() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let memory = recording_memory();
        memory
            .remember(
                &MemoryScope::new("team:alpha").unwrap(),
                MemoryWrite::unexplained(vec![remembered_decision(
                    "roll back rather than restart",
                )])
                .unwrap(),
                "seed",
            )
            .await
            .unwrap();
        let (stream, store) = stream_over(Arc::new(EventStoreFake::default()));
        let usecase = StartCeremonyUseCase::new(
            definitions,
            stream,
            Arc::new(FixedClock::new(now())),
            memory,
        );

        let instance = usecase
            .execute(StartCeremonyInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                shared_memory("team:alpha"),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        let recalled = instance
            .recollection()
            .expect("the session was told what the scope holds");
        assert_eq!(recalled.scope().as_str(), "team:alpha");
        assert_eq!(
            recalled.entries()[0].summary(),
            "roll back rather than restart"
        );

        let facts = store.facts().await;
        assert_eq!(facts.len(), 2, "{facts:?}");
        assert_eq!(
            facts[1].event.event_type(),
            AuditEventType::MemoryRecalled,
            "the recollection is sealed right after the opening"
        );
        assert_eq!(
            facts[1].correlation_id.as_ref(),
            Some(&facts[0].event_id),
            "the recollection belongs to the session it opened with"
        );
        assert_eq!(
            facts[1].causation_id.as_ref(),
            Some(&facts[0].event_id),
            "the opening is what caused the recollection"
        );
    }

    /// Memory is not the transaction. A backend that is out costs the
    /// session what it would have been told, and nothing else.
    #[tokio::test]
    async fn a_memory_that_cannot_be_read_does_not_stop_a_session() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let (stream, store) = stream_over(Arc::new(EventStoreFake::default()));
        let usecase = StartCeremonyUseCase::new(
            definitions,
            stream,
            Arc::new(FixedClock::new(now())),
            Arc::new(MemoryThatIsOut),
        );

        let instance = usecase
            .execute(StartCeremonyInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                shared_memory("team:alpha"),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .expect("a memory that is out must not fail a start");

        assert!(instance.recollection().is_none());
        assert_eq!(store.facts().await.len(), 1);
    }

    /// A scope the caller declared and got wrong is a caller's mistake.
    /// Falling back to the private default would hand them a session
    /// that remembers alone while they believe it is sharing.
    #[tokio::test]
    async fn a_declared_scope_that_is_not_a_scope_opens_nothing() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let usecase = StartCeremonyUseCase::new(
            definitions,
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
            a_memory(),
        );

        usecase
            .execute(StartCeremonyInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                shared_memory("alpha"),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap_err();

        assert!(
            !instances.exists(&ceremony_id()).await,
            "a session was opened under a scope the caller cannot have meant"
        );
    }

    fn shared_memory(scope: &str) -> CeremonyContext {
        CeremonyContext::new(
            Attributes::new(
                [("memory_scope".to_owned(), serde_json::json!(scope))]
                    .into_iter()
                    .collect(),
            )
            .unwrap(),
        )
    }

    /// A session opened badly leaves nothing behind.
    ///
    /// The fact is built before the commit, so a caller who names
    /// themselves with something the journal will not accept is refused
    /// without a session existing that has no record of being opened.
    #[tokio::test]
    async fn a_caller_who_cannot_be_named_opens_nothing() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let usecase = StartCeremonyUseCase::new(
            definitions,
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
            a_memory(),
        );

        usecase
            .execute(StartCeremonyInput::new(
                ceremony_id(),
                definition_name(),
                version(),
                CeremonyContext::empty(),
                "   ",
                AuditActorKind::Service,
            ))
            .await
            .unwrap_err();

        assert!(
            !instances.exists(&ceremony_id()).await,
            "a session was opened that the stream cannot account for"
        );
    }
}
