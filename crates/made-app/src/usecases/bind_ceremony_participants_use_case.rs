//! [`BindCeremonyParticipantsUseCase`] — who sits at this table.
//!
//! A definition says what each role does. Seating says who is doing
//! it in this session, and it is per session on purpose: the same
//! ceremony run twice can and should be able to seat different people.

use std::sync::Arc;

use super::bind_ceremony_participants_input::BindCeremonyParticipantsInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

use made_core::entities::ceremony_commands::BindParticipant;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;
#[cfg(test)]
use made_core::value_objects::{AuditActorKind, Specialty};

pub struct BindCeremonyParticipantsUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for BindCeremonyParticipantsUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BindCeremonyParticipantsUseCase").finish()
    }
}

impl BindCeremonyParticipantsUseCase {
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
        name = "bind_ceremony_participants",
        skip_all,
        fields(ceremony_id = %input.instance_id)
    )]
    pub async fn execute(
        &self,
        input: BindCeremonyParticipantsInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::party(&input.actor_id, input.actor_kind)?;
        let now = self.clock.now();
        let commands = input
            .seating
            .iter()
            .map(|(role_id, specialty)| {
                CeremonyCommand::BindParticipant(BindParticipant {
                    role_id: role_id.clone(),
                    specialty: specialty.clone(),
                    now,
                })
            })
            .collect::<Vec<_>>();

        // All of it or none of it: one seat is one command, decided
        // in turn against the seating so far, and the facts of every
        // seat land in one append. A seat the ceremony never declared
        // stops the call before anything is saved — a caller seating
        // three roles and getting two would have to work out which,
        // and a half-seated table is not something anyone asked for.
        self.stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let mut seated = session.instance.clone();
                let mut events = Vec::new();
                for command in &commands {
                    let decided = seated.decide(command, &definition)?;
                    for event in &decided {
                        seated.apply(event);
                    }
                    events.extend(decided);
                }
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await
            .map(|session| session.instance)
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::CeremonyEvent;
    use made_core::value_objects::AuditEventType;

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, now, role_id, started_instance, stream_over,
        DefinitionRepositoryFake, EventStoreFake, FixedClock,
    };

    async fn seated() -> (Arc<DefinitionRepositoryFake>, Arc<EventStoreFake>) {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        (definitions, instances)
    }

    /// Seating the table is a fact about the session, and the seater
    /// holds no seat in it.
    #[tokio::test]
    async fn seals_the_seating_into_the_journal() {
        let (definitions, instances) = seated().await;
        let (stream, store) = stream_over(instances);
        let usecase = BindCeremonyParticipantsUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(
                BindCeremonyParticipantsInput::new(
                    ceremony_id(),
                    [(role_id(), Specialty::new("reviewer").unwrap())],
                    "operator-1",
                    AuditActorKind::Human,
                )
                .unwrap(),
            )
            .await
            .unwrap();

        let facts = store.facts().await;
        assert_eq!(facts.len(), 1, "one seating, one fact: {facts:?}");
        assert_eq!(
            facts[0].event.event_type(),
            AuditEventType::ParticipantsBound
        );
        let CeremonyEvent::ParticipantsBound(seated) = &facts[0].event else {
            panic!("a seating seals who sits where: {:?}", facts[0].event);
        };
        assert_eq!(seated.bindings.len(), 1);
        assert_eq!(seated.bindings[0].role_id(), &role_id());
        assert_eq!(seated.bindings[0].specialty().as_str(), "reviewer");
        assert_eq!(facts[0].actor.kind(), AuditActorKind::Human);
        assert!(
            facts[0].actor.role_id().is_none(),
            "the seater was given a seat this ceremony never assigned"
        );
    }

    /// Seating the same role somewhere else is a second fact.
    ///
    /// The id is the seating itself, so this is the case that decides
    /// whether the scheme works: a role moved to another specialty must
    /// not derive the id of where it used to sit.
    #[tokio::test]
    async fn re_seating_a_role_elsewhere_is_a_distinct_fact() {
        let (definitions, instances) = seated().await;
        let (stream, store) = stream_over(instances);
        let usecase = BindCeremonyParticipantsUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        for specialty in ["reviewer", "auditor"] {
            usecase
                .execute(
                    BindCeremonyParticipantsInput::new(
                        ceremony_id(),
                        [(role_id(), Specialty::new(specialty).unwrap())],
                        "operator-1",
                        AuditActorKind::Human,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
        }

        let ids = store
            .facts()
            .await
            .iter()
            .map(|fact| fact.event_id.as_str().to_owned())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            ids.len(),
            2,
            "moving a role to another specialty derived the id of where it used to sit: {ids:?}"
        );
    }
}
