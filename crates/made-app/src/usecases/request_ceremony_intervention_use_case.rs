//! [`RequestCeremonyInterventionUseCase`] — add a live agenda item.

use std::sync::Arc;

use made_core::entities::ceremony_commands::RequestIntervention;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::request_ceremony_intervention_input::RequestCeremonyInterventionInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct RequestCeremonyInterventionUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for RequestCeremonyInterventionUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestCeremonyInterventionUseCase")
            .finish()
    }
}

impl RequestCeremonyInterventionUseCase {
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
        name = "request_ceremony_intervention",
        skip_all,
        fields(
            ceremony_id = %input.instance_id,
            intervention_id = %input.intervention_id,
            role_id = %input.role_id,
        )
    )]
    pub async fn execute(
        &self,
        input: RequestCeremonyInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        // Resolved from the instance, never from the request: a session
        // bound to a published version must be advanced by the very
        // definition it recorded, and one that is unbound has only the
        // repository to go to.
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::seat(&input.role_id, input.role_kind)?;
        let now = self.clock.now();
        let command = CeremonyCommand::RequestIntervention(RequestIntervention {
            intervention_id: input.intervention_id,
            role_id: input.role_id,
            kind: input.kind,
            target: input.target,
            content: input.content,
            provenance: input.provenance,
            now,
        });
        // An item asked of the table commutes with what other writers
        // do to the session, so a lost race is decided again.
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
    use made_core::entities::CeremonyEvent;
    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, CeremonyInterventionContent,
        CeremonyInterventionId, CeremonyInterventionKind, CeremonyInterventionTarget,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, now, role_id, started_instance, stream,
        stream_over, DefinitionRepositoryFake, EventStoreFake, FixedClock,
    };

    #[tokio::test]
    async fn persists_a_dynamic_intervention_on_the_running_instance() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let usecase = RequestCeremonyInterventionUseCase::new(
            definition_resolver(definitions),
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
        );
        let intervention_id = CeremonyInterventionId::new("ask-table").unwrap();

        let instance = usecase
            .execute(RequestCeremonyInterventionInput::new(
                ceremony_id(),
                intervention_id.clone(),
                role_id(),
                AuditActorKind::Human,
                CeremonyInterventionKind::Opinion,
                CeremonyInterventionTarget::table(),
                CeremonyInterventionContent::new("What does the table think?", Attributes::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();

        assert_eq!(
            instance
                .intervention(&intervention_id)
                .unwrap()
                .requested_by(),
            &role_id()
        );
        assert!(instances
            .saved(&ceremony_id())
            .await
            .intervention(&intervention_id)
            .is_some());
    }

    /// Asking the table for something is a fact about the session.
    ///
    /// The agenda item itself says what was asked and by which seat.
    /// What it cannot say is that somebody asked at a moment — that is
    /// the journal's, and it is what a reader reconstructing the
    /// session's shape follows.
    #[tokio::test]
    async fn seals_the_request_into_the_journal() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let (stream, store) = stream_over(instances);
        let usecase = RequestCeremonyInterventionUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(RequestCeremonyInterventionInput::new(
                ceremony_id(),
                CeremonyInterventionId::new("ask-table").unwrap(),
                role_id(),
                AuditActorKind::Agent,
                CeremonyInterventionKind::Opinion,
                CeremonyInterventionTarget::table(),
                CeremonyInterventionContent::new("What does the table think?", Attributes::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();

        let facts = store.facts().await;
        assert_eq!(facts.len(), 1, "one request, one fact: {facts:?}");
        assert_eq!(
            facts[0].event.event_type(),
            AuditEventType::InterventionRequested
        );
        let CeremonyEvent::InterventionRequested(requested) = &facts[0].event else {
            panic!("a request seals the item it opened: {:?}", facts[0].event);
        };
        assert_eq!(requested.intervention.requested_by(), &role_id());
        assert_eq!(
            requested.intervention.request().message(),
            "What does the table think?"
        );
        // Declared by the caller and carried through. A guard requiring
        // a human says one was required; this says an agent asked.
        assert_eq!(facts[0].actor.kind(), AuditActorKind::Agent);
    }

    /// Two agenda items are two facts, not one written twice.
    ///
    /// The event id is derived, so it has to derive from something
    /// that differs between requests. Keyed on the session alone, the
    /// second ask would collide with the first and a reader would see
    /// one request where there were two.
    #[tokio::test]
    async fn two_requests_are_two_distinct_facts() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let (stream, store) = stream_over(instances);
        let usecase = RequestCeremonyInterventionUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        for asked in ["ask-one", "ask-two"] {
            usecase
                .execute(RequestCeremonyInterventionInput::new(
                    ceremony_id(),
                    CeremonyInterventionId::new(asked).unwrap(),
                    role_id(),
                    AuditActorKind::Agent,
                    CeremonyInterventionKind::Opinion,
                    CeremonyInterventionTarget::table(),
                    CeremonyInterventionContent::new(asked, Attributes::empty()).unwrap(),
                ))
                .await
                .unwrap();
        }

        let ids = store
            .facts()
            .await
            .iter()
            .map(|fact| fact.event_id.as_str().to_owned())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), 2, "two asks collapsed into one fact: {ids:?}");
    }
}
