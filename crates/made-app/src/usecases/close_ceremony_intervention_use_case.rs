//! [`CloseCeremonyInterventionUseCase`] — close a live agenda item.

use std::sync::Arc;

use made_core::entities::ceremony_commands::CloseIntervention;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::close_ceremony_intervention_input::CloseCeremonyInterventionInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct CloseCeremonyInterventionUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for CloseCeremonyInterventionUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloseCeremonyInterventionUseCase").finish()
    }
}

impl CloseCeremonyInterventionUseCase {
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

    #[tracing::instrument(
        name = "close_ceremony_intervention",
        skip_all,
        fields(
            ceremony_id = %input.instance_id,
            intervention_id = %input.intervention_id,
            role_id = %input.role_id,
        )
    )]
    pub async fn execute(
        &self,
        input: CloseCeremonyInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        // Resolved from the instance, never from the request: a session
        // bound to a published version must be advanced by the very
        // definition it recorded, and one that is unbound has only the
        // repository to go to.
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::seat(&input.role_id, input.role_kind)?;
        let now = self.clock.now();
        let command = CeremonyCommand::CloseIntervention(CloseIntervention {
            intervention_id: input.intervention_id,
            role_id: input.role_id,
            now,
        });
        // Closing an item commutes with what other writers do to the
        // session, so a lost race is decided again; an item that was
        // closed meanwhile is refused by the decision, not the store.
        self.stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&command, &definition)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await
            .map(|session| session.instance)
    }
}

#[cfg(test)]
mod tests {
    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, CeremonyInterventionContent,
        CeremonyInterventionId, CeremonyInterventionKind, CeremonyInterventionStatus,
        CeremonyInterventionTarget,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, now, role_id, started_instance, stream,
        stream_over, DefinitionRepositoryFake, EventStoreFake, FixedClock,
    };

    #[tokio::test]
    async fn requester_closes_the_dynamic_agenda_item() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let intervention_id = CeremonyInterventionId::new("ask-table").unwrap();
        let mut instance = started_instance(&definition);
        instance
            .request_intervention_as(
                &definition,
                intervention_id.clone(),
                role_id(),
                CeremonyInterventionKind::Opinion,
                CeremonyInterventionTarget::table(),
                CeremonyInterventionContent::new("What do you think?", Attributes::empty())
                    .unwrap(),
                now(),
            )
            .unwrap();
        instances.save(&instance).await.unwrap();
        let usecase = CloseCeremonyInterventionUseCase::new(
            definition_resolver(definitions),
            stream(instances),
            Arc::new(FixedClock::new(now())),
        );

        let instance = usecase
            .execute(CloseCeremonyInterventionInput::new(
                ceremony_id(),
                intervention_id.clone(),
                role_id(),
                AuditActorKind::Human,
            ))
            .await
            .unwrap();

        assert_eq!(
            instance.intervention(&intervention_id).unwrap().status(),
            CeremonyInterventionStatus::Closed
        );
    }

    /// A session with an open agenda item, ready to have it closed.
    async fn with_an_open_item() -> (
        Arc<DefinitionRepositoryFake>,
        Arc<EventStoreFake>,
        CeremonyInterventionId,
    ) {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let intervention_id = CeremonyInterventionId::new("ask-table").unwrap();
        let mut instance = started_instance(&definition);
        instance
            .request_intervention_as(
                &definition,
                intervention_id.clone(),
                role_id(),
                CeremonyInterventionKind::Opinion,
                CeremonyInterventionTarget::table(),
                CeremonyInterventionContent::new("What do you think?", Attributes::empty())
                    .unwrap(),
                now(),
            )
            .unwrap();
        instances.save(&instance).await.unwrap();
        (definitions, instances, intervention_id)
    }

    /// Closing is a decision, and it leaves a record of who made it.
    #[tokio::test]
    async fn seals_the_closure_into_the_journal() {
        let (definitions, instances, intervention_id) = with_an_open_item().await;
        let (stream, store) = stream_over(instances);
        let usecase = CloseCeremonyInterventionUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(CloseCeremonyInterventionInput::new(
                ceremony_id(),
                intervention_id,
                role_id(),
                AuditActorKind::Human,
            ))
            .await
            .unwrap();

        let facts = store.facts().await;
        assert_eq!(facts.len(), 1, "one closure, one fact: {facts:?}");
        assert_eq!(
            facts[0].event.event_type(),
            AuditEventType::InterventionClosed
        );
        assert_eq!(facts[0].actor.kind(), AuditActorKind::Human);
    }

    /// Why the closure's id needs no seat, unlike a response's.
    ///
    /// A response is keyed on the answering seat because an item put to
    /// the table is answered by several. A closure is not: the session
    /// refuses a second one, so the item alone identifies it and a
    /// retry derives the same id rather than a second entry.
    ///
    /// This is the assertion behind that comment. Without it, the day
    /// closing twice becomes legal the journal would quietly record the
    /// second closure under the first one's id and lose it.
    #[tokio::test]
    async fn an_item_is_closed_once_and_the_session_says_so() {
        let (definitions, instances, intervention_id) = with_an_open_item().await;
        let (stream, store) = stream_over(instances);
        let usecase = CloseCeremonyInterventionUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );
        let closing = || {
            usecase.execute(CloseCeremonyInterventionInput::new(
                ceremony_id(),
                intervention_id.clone(),
                role_id(),
                AuditActorKind::Human,
            ))
        };

        closing().await.unwrap();
        let refused = closing().await;

        assert!(
            refused.is_err(),
            "closing an already-closed item was accepted, and its fact would collide with the \
             first: {refused:?}"
        );
        assert_eq!(
            store.facts().await.len(),
            1,
            "a refused closure sealed a fact"
        );
    }
}
