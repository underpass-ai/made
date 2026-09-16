//! [`AssertCeremonyReasonUseCase`] — say why one thing here led to another.

use std::sync::Arc;

use made_core::entities::ceremony_commands::AssertReason;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;
use made_core::value_objects::CeremonyReason;

use super::assert_ceremony_reason_input::AssertCeremonyReasonInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, ConflictPolicy, SessionMemoryRecorder, SessionStream};

pub struct AssertCeremonyReasonUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
    memory: Arc<SessionMemoryRecorder>,
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
        memory: Arc<SessionMemoryRecorder>,
    ) -> Self {
        Self {
            definitions,
            stream,
            clock,
            memory,
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
        // The reason a later session will follow, sent on once the
        // session that holds it is safely stored.
        self.memory
            .remember_reason(&instance, instance.reasons().len().saturating_sub(1))
            .await;
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
        ceremony_id, definition, definition_resolver, now, recorder, recording_memory,
        respondent_role_id, role_id, started_instance, stream, stream_over,
        DefinitionRepositoryFake, EventStoreFake, FixedClock,
    };
    use crate::usecases::{
        RespondToCeremonyInterventionInput, RespondToCeremonyInterventionUseCase,
    };

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

        let mut instance = started_instance(&definition);
        for id in [
            &agenda_item,
            &CeremonyInterventionId::new("what-next").unwrap(),
        ] {
            instance
                .request_intervention_as(
                    &definition,
                    id.clone(),
                    role_id(),
                    CeremonyInterventionKind::Investigation,
                    CeremonyInterventionTarget::roles([respondent_role_id()]).unwrap(),
                    CeremonyInterventionContent::new("Look.", Attributes::empty()).unwrap(),
                    now(),
                )
                .unwrap();
        }
        instances.save(&instance).await.unwrap();

        let respond = RespondToCeremonyInterventionUseCase::new(
            definition_resolver(definitions.clone()),
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
            recorder(memory.clone()),
        );
        let later = CeremonyInterventionId::new("what-next").unwrap();
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
            stream(instances),
            Arc::new(FixedClock::new(now())),
            recorder(memory.clone()),
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

        let mut instance = started_instance(&definition);
        instance
            .request_intervention_as(
                &definition,
                agenda_item.clone(),
                role_id(),
                CeremonyInterventionKind::Investigation,
                CeremonyInterventionTarget::roles([respondent_role_id()]).unwrap(),
                CeremonyInterventionContent::new("Look.", Attributes::empty()).unwrap(),
                now(),
            )
            .unwrap();
        instances.save(&instance).await.unwrap();

        RespondToCeremonyInterventionUseCase::new(
            definition_resolver(definitions.clone()),
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
            recorder(memory.clone()),
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
            stream(instances),
            Arc::new(FixedClock::new(now())),
            recorder(memory.clone()),
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

        let mut instance = started_instance(&definition);
        for id in [&agenda_item, &later] {
            instance
                .request_intervention_as(
                    &definition,
                    id.clone(),
                    role_id(),
                    CeremonyInterventionKind::Investigation,
                    CeremonyInterventionTarget::roles([respondent_role_id()]).unwrap(),
                    CeremonyInterventionContent::new("Look.", Attributes::empty()).unwrap(),
                    now(),
                )
                .unwrap();
        }
        instances.save(&instance).await.unwrap();
        let (stream, store) = stream_over(instances);

        let respond = RespondToCeremonyInterventionUseCase::new(
            definition_resolver(definitions.clone()),
            stream.clone(),
            Arc::new(FixedClock::new(now())),
            recorder(memory.clone()),
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
            recorder(memory),
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
