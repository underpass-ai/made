//! [`RespondToCeremonyInterventionUseCase`] — contribute to a live agenda item.

use std::sync::Arc;

use made_core::entities::ceremony_commands::RespondToIntervention;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use super::respond_to_ceremony_intervention_input::RespondToCeremonyInterventionInput;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct RespondToCeremonyInterventionUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for RespondToCeremonyInterventionUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RespondToCeremonyInterventionUseCase")
            .finish()
    }
}

impl RespondToCeremonyInterventionUseCase {
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
        name = "respond_to_ceremony_intervention",
        skip_all,
        fields(
            ceremony_id = %input.instance_id,
            intervention_id = %input.intervention_id,
            role_id = %input.role_id,
        )
    )]
    pub async fn execute(
        &self,
        input: RespondToCeremonyInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        // Resolved from the instance, never from the request: a session
        // bound to a published version must be advanced by the very
        // definition it recorded, and one that is unbound has only the
        // repository to go to.
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::seat(&input.role_id, input.role_kind)?;
        let now = self.clock.now();
        let command = CeremonyCommand::RespondToIntervention(RespondToIntervention {
            intervention_id: input.intervention_id.clone(),
            role_id: input.role_id,
            content: input.content,
            now,
        });
        // A contribution commutes with what other writers do to the
        // session — another seat answering the same item most of all —
        // so a lost race is decided again.
        let instance = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&command, &definition)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await?
            .instance;
        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, CeremonyInterventionContent,
        CeremonyInterventionId, CeremonyInterventionKind, CeremonyInterventionTarget,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, now, respondent_role_id, role_id,
        started_instance, stream, stream_over, DefinitionRepositoryFake, EventStoreFake,
        FixedClock,
    };
    use crate::usecases::{RequestCeremonyInterventionInput, RequestCeremonyInterventionUseCase};

    #[tokio::test]
    async fn persists_a_table_members_response() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let intervention_id = CeremonyInterventionId::new("inspect-queue").unwrap();
        let mut instance = started_instance(&definition);
        instance
            .request_intervention_as(
                &definition,
                intervention_id.clone(),
                role_id(),
                CeremonyInterventionKind::Investigation,
                CeremonyInterventionTarget::roles([respondent_role_id()]).unwrap(),
                CeremonyInterventionContent::new("Inspect the queue.", Attributes::empty())
                    .unwrap(),
                now(),
            )
            .unwrap();
        instances.save(&instance).await.unwrap();
        let usecase = RespondToCeremonyInterventionUseCase::new(
            definition_resolver(definitions),
            stream(instances),
            Arc::new(FixedClock::new(now())),
        );

        let instance = usecase
            .execute(RespondToCeremonyInterventionInput::new(
                ceremony_id(),
                intervention_id.clone(),
                respondent_role_id(),
                AuditActorKind::Agent,
                CeremonyInterventionContent::new("Queue depth is stable.", Attributes::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();

        assert_eq!(
            instance
                .intervention(&intervention_id)
                .unwrap()
                .responses()
                .len(),
            1
        );
    }

    /// Answering is a fact about the session, distinct from asking.
    ///
    /// A request and its answer collapsing into one entry would lose
    /// the shape a reader follows: what was asked, and separately that
    /// somebody came back with something.
    #[tokio::test]
    async fn seals_the_response_apart_from_the_request() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let (stream, store) = stream_over(instances.clone());
        let intervention_id = CeremonyInterventionId::new("ask-table").unwrap();
        RequestCeremonyInterventionUseCase::new(
            definition_resolver(definitions.clone()),
            stream.clone(),
            Arc::new(FixedClock::new(now())),
        )
        .execute(RequestCeremonyInterventionInput::new(
            ceremony_id(),
            intervention_id.clone(),
            role_id(),
            AuditActorKind::Human,
            CeremonyInterventionKind::Opinion,
            CeremonyInterventionTarget::roles([respondent_role_id()]).unwrap(),
            CeremonyInterventionContent::new("What does the table think?", Attributes::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

        RespondToCeremonyInterventionUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        )
        .execute(RespondToCeremonyInterventionInput::new(
            ceremony_id(),
            intervention_id,
            respondent_role_id(),
            AuditActorKind::Agent,
            CeremonyInterventionContent::new("Queue depth is stable.", Attributes::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

        let facts = store.facts().await;
        let sealed = facts
            .iter()
            .map(|fact| fact.event.event_type())
            .collect::<Vec<_>>();
        assert_eq!(
            sealed,
            vec![
                AuditEventType::InterventionRequested,
                AuditEventType::InterventionResponded
            ],
            "asking and answering are two facts: {facts:?}"
        );
        // Two different parties, two different declared kinds, neither
        // deduced from the other.
        assert_eq!(facts[0].actor.kind(), AuditActorKind::Human);
        assert_eq!(facts[1].actor.kind(), AuditActorKind::Agent);
        // The answering seat is part of what identifies the fact. An
        // item put to the whole table is answered by more than one of
        // them, and keyed on the item alone the second answer would
        // derive the first one's id and be lost.
        assert!(
            facts[1]
                .event_id
                .as_str()
                .contains(respondent_role_id().as_str()),
            "the answering seat is not part of the fact's identity: {}",
            facts[1].event_id.as_str()
        );
    }
}
