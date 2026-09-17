//! [`AssertCeremonyReasonUseCase`] — say why one thing here led to another.

use std::sync::Arc;

use made_core::entities::ceremony_commands::AssertReason;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;
use made_core::value_objects::CeremonyReason;

use super::assert_ceremony_reason_input::AssertCeremonyReasonInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct AssertCeremonyReasonUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for AssertCeremonyReasonUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssertCeremonyReasonUseCase").finish()
    }
}

impl AssertCeremonyReasonUseCase {
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
        name = "assert_ceremony_reason",
        skip_all,
        fields(
            ceremony_id = %input.instance_id,
            role_id = %input.role_id,
            kind = input.kind.as_label(),
        )
    )]
    pub async fn execute(
        &self,
        input: AssertCeremonyReasonInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::seat(&input.role_id, input.role_kind)?;
        let now = self.clock.now();
        let command = CeremonyCommand::AssertReason(AssertReason {
            reason: CeremonyReason::new(
                input.from,
                input.to,
                input.kind,
                input.why,
                input.confidence,
                Some(input.role_id),
                now,
            )?,
        });
        // A judgement commutes with what other writers do to the
        // session, so a lost race is decided again — and numbered
        // again, since its id is its position among the reasons.
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
        CeremonyReasonKind, CeremonyRecordRef, MemoryConfidence,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, now, recording_memory, remembering_stream,
        respondent_role_id, role_id, started_instance, DefinitionRepositoryFake, EventStoreFake,
        FixedClock,
    };
    use crate::usecases::{
        RequestCeremonyInterventionInput, RequestCeremonyInterventionUseCase,
        RespondToCeremonyInterventionInput, RespondToCeremonyInterventionUseCase,
    };

    /// Put these items on the session's table, through the use case
    /// that does it.
    ///
    /// Asked rather than seeded, because memory is a projection of the
    /// stream: an item a test wrote straight into a snapshot is state
    /// no record accounts for, and the real store would only hold it
    /// after the events that produced it.
    async fn ask_about<'a>(
        stream: &Arc<SessionStream>,
        definitions: Arc<DefinitionRepositoryFake>,
        items: impl IntoIterator<Item = &'a CeremonyInterventionId>,
    ) {
        let usecase = RequestCeremonyInterventionUseCase::new(
            definition_resolver(definitions),
            stream.clone(),
            Arc::new(FixedClock::new(now())),
        );
        for item in items {
            usecase
                .execute(RequestCeremonyInterventionInput::new(
                    ceremony_id(),
                    item.clone(),
                    role_id(),
                    AuditActorKind::Human,
                    CeremonyInterventionKind::Investigation,
                    CeremonyInterventionTarget::roles([respondent_role_id()]).unwrap(),
                    CeremonyInterventionContent::new("Look.", Attributes::empty()).unwrap(),
                ))
                .await
                .unwrap();
        }
    }

    /// The whole point, end to end: a session contributes, explains
    /// itself, and memory receives the entry **and the edge**.
    ///
    /// The edge is the part that was impossible until now. Without it
    /// a later session can read what was said and never work out what
    /// made anyone say it.
    #[tokio::test]
    async fn a_reason_reaches_memory_as_an_edge_between_two_entries() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let memory = recording_memory();
        let agenda_item = CeremonyInterventionId::new("inspect-queue").unwrap();

        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let stream = remembering_stream(instances.clone(), memory.clone());
        let later = CeremonyInterventionId::new("what-next").unwrap();
        // Asked through the use case, so the items are events of the
        // stream rather than state only a seeded snapshot holds: what
        // memory is a projection of is the stream.
        ask_about(&stream, definitions.clone(), [&agenda_item, &later]).await;

        let respond = RespondToCeremonyInterventionUseCase::new(
            definition_resolver(definitions.clone()),
            stream.clone(),
            Arc::new(FixedClock::new(now())),
        );
        for (id, said) in [
            (&agenda_item, "the queue was backing up"),
            (&later, "roll back rather than restart"),
        ] {
            respond
                .execute(RespondToCeremonyInterventionInput::new(
                    ceremony_id(),
                    id.clone(),
                    respondent_role_id(),
                    AuditActorKind::Agent,
                    CeremonyInterventionContent::new(said, Attributes::empty()).unwrap(),
                ))
                .await
                .unwrap();
        }

        AssertCeremonyReasonUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        )
        .execute(AssertCeremonyReasonInput::new(
            ceremony_id(),
            respondent_role_id(),
            AuditActorKind::Agent,
            CeremonyRecordRef::contribution(later, 0),
            CeremonyRecordRef::contribution(agenda_item, 0),
            CeremonyReasonKind::ChosenBecause,
            "the queue growth is what made a rollback necessary",
            MemoryConfidence::High,
        ))
        .await
        .unwrap();

        let entries = memory.entries().await;
        assert_eq!(entries.len(), 2, "both contributions were remembered");

        let relations = memory.relations().await;
        let [edge] = relations.as_slice() else {
            panic!("expected exactly one edge, got {relations:?}");
        };
        assert_eq!(
            edge.why(),
            "the queue growth is what made a rollback necessary"
        );
        assert_eq!(edge.confidence(), MemoryConfidence::High);
        assert_eq!(edge.from().as_str(), "agenda:what-next:contribution:0");
        assert_eq!(edge.to().as_str(), "agenda:inspect-queue:contribution:0");
    }

    /// A reason whose end was never remembered is not sent.
    ///
    /// A step is machinery and memory keeps no kind for it, so an edge
    /// into one would claim an explanation exists and give no way to
    /// reach it.
    #[tokio::test]
    async fn a_reason_into_something_unremembered_is_not_sent() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let memory = recording_memory();
        let agenda_item = CeremonyInterventionId::new("inspect-queue").unwrap();

        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let stream = remembering_stream(instances.clone(), memory.clone());
        ask_about(&stream, definitions.clone(), [&agenda_item]).await;

        RespondToCeremonyInterventionUseCase::new(
            definition_resolver(definitions.clone()),
            stream.clone(),
            Arc::new(FixedClock::new(now())),
        )
        .execute(RespondToCeremonyInterventionInput::new(
            ceremony_id(),
            agenda_item.clone(),
            respondent_role_id(),
            AuditActorKind::Agent,
            CeremonyInterventionContent::new("the queue was backing up", Attributes::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

        AssertCeremonyReasonUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        )
        .execute(AssertCeremonyReasonInput::new(
            ceremony_id(),
            respondent_role_id(),
            AuditActorKind::Agent,
            CeremonyRecordRef::contribution(agenda_item, 0),
            CeremonyRecordRef::agenda_item(CeremonyInterventionId::new("inspect-queue").unwrap()),
            CeremonyReasonKind::FollowsFrom,
            "the item is where the finding came from",
            MemoryConfidence::Low,
        ))
        .await
        .unwrap();

        assert!(
            memory.relations().await.is_empty(),
            "an edge into something memory never kept was sent anyway"
        );
    }

    /// The same edge said twice is two claims, not one written again.
    ///
    /// Nothing stops a session holding two reasons between the same
    /// pair: two seats can reach the same conclusion, and one seat can
    /// say it again with a different why. So the fact's id derives from
    /// the reason's position, not from the edge — keyed on the edge,
    /// the second claim would derive the first one's id and vanish.
    ///
    /// This is the assertion behind that choice. The day the session
    /// starts refusing a duplicate edge, this fails and the key can be
    /// reconsidered on purpose rather than by accident.
    #[tokio::test]
    async fn a_repeated_edge_is_sealed_as_a_second_claim() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let memory = recording_memory();
        let agenda_item = CeremonyInterventionId::new("inspect-queue").unwrap();
        let later = CeremonyInterventionId::new("what-next").unwrap();

        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let store = instances.clone();
        let stream = remembering_stream(instances, memory.clone());
        ask_about(&stream, definitions.clone(), [&agenda_item, &later]).await;

        let respond = RespondToCeremonyInterventionUseCase::new(
            definition_resolver(definitions.clone()),
            stream.clone(),
            Arc::new(FixedClock::new(now())),
        );
        for (id, said) in [
            (&agenda_item, "the queue was backing up"),
            (&later, "roll back rather than restart"),
        ] {
            respond
                .execute(RespondToCeremonyInterventionInput::new(
                    ceremony_id(),
                    id.clone(),
                    respondent_role_id(),
                    AuditActorKind::Agent,
                    CeremonyInterventionContent::new(said, Attributes::empty()).unwrap(),
                ))
                .await
                .unwrap();
        }

        let usecase = AssertCeremonyReasonUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );
        for why in [
            "the queue growth made it necessary",
            "and nothing else would do",
        ] {
            usecase
                .execute(AssertCeremonyReasonInput::new(
                    ceremony_id(),
                    respondent_role_id(),
                    AuditActorKind::Agent,
                    CeremonyRecordRef::contribution(later.clone(), 0),
                    CeremonyRecordRef::contribution(agenda_item.clone(), 0),
                    CeremonyReasonKind::ChosenBecause,
                    why,
                    MemoryConfidence::High,
                ))
                .await
                .unwrap();
        }

        let reasons = store
            .facts()
            .await
            .into_iter()
            .filter(|fact| fact.event.event_type() == AuditEventType::ReasonAsserted)
            .collect::<Vec<_>>();
        assert_eq!(reasons.len(), 2, "two claims, two facts: {reasons:?}");
        assert_ne!(
            reasons[0].event_id.as_str(),
            reasons[1].event_id.as_str(),
            "the second claim derived the first one's id and would be lost"
        );
        assert!(reasons
            .iter()
            .all(|fact| fact.actor.kind() == AuditActorKind::Agent));
    }
}
