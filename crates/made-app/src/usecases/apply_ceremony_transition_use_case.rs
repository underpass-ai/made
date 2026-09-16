//! [`ApplyCeremonyTransitionUseCase`] — apply a guarded ceremony transition.

use std::sync::Arc;

use made_core::entities::ceremony_commands::ApplyTransition;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::apply_ceremony_transition_input::ApplyCeremonyTransitionInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct ApplyCeremonyTransitionUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for ApplyCeremonyTransitionUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplyCeremonyTransitionUseCase").finish()
    }
}

impl ApplyCeremonyTransitionUseCase {
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
        name = "apply_ceremony_transition",
        skip_all,
        fields(ceremony_id = %input.instance_id, trigger = %input.trigger)
    )]
    pub async fn execute(
        &self,
        input: ApplyCeremonyTransitionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        // Resolved from the instance, never from the request: a session
        // bound to a published version must be advanced by the very
        // definition it recorded, and one that is unbound has only the
        // repository to go to. Reading coordinates off the caller made
        // a bound session unadvanceable, because publishing writes to
        // the catalogue and not to the repository.
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::seat(&input.role_id, input.role_kind)?;
        let now = self.clock.now();
        let command = CeremonyCommand::ApplyTransition(ApplyTransition {
            role_id: Some(input.role_id),
            trigger: input.trigger,
            now,
        });
        // Fail fast, on purpose. A move changes which commands are
        // legal next, so a caller whose session moved under them
        // decided against a state that is gone; they are told, rather
        // than moved on their behalf. The move and the state it moved
        // to still land together: a crash between them would leave a
        // session sitting in a state no recorded move accounts for.
        let instance = self
            .stream
            .execute(session, ConflictPolicy::FailFast, |session| {
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
    use made_core::entities::CeremonyEvent;
    use std::sync::Arc;

    use made_core::error::DomainError;
    use made_core::value_objects::{
        AuditActorKind, AuditEventType, StateId, StepOutput, StepResult,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, now, recording_memory, remembering_stream,
        role_id, started_instance, step_id, stream, stream_conflicting_once,
        stream_losing_every_race, stream_over, trigger, DefinitionRepositoryFake, EventStoreFake,
        FixedClock,
    };

    #[tokio::test]
    async fn applies_guarded_transition_after_step_completion() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let mut instance = started_instance(&definition);
        instance
            .start_step(
                &definition,
                &step_id(),
                made_core::value_objects::StepLease::new(
                    crate::usecases::ceremony_test_support::lease_owner(),
                    crate::usecases::ceremony_test_support::idempotency_key("lease-1"),
                    now(),
                    now() + time::Duration::seconds(60),
                )
                .unwrap(),
                now(),
            )
            .unwrap();
        instance
            .apply_step_result(
                &definition,
                &step_id(),
                StepResult::completed(StepOutput::empty()).unwrap(),
                now(),
            )
            .unwrap();
        instances.save(&instance).await.unwrap();
        let usecase = ApplyCeremonyTransitionUseCase::new(
            definition_resolver(definitions),
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
        );

        let transitioned = usecase
            .execute(ApplyCeremonyTransitionInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                trigger(),
            ))
            .await
            .unwrap();

        assert_eq!(
            transitioned.current_state(),
            &StateId::new("COMPLETED").unwrap()
        );
        assert!(instances
            .saved(&ceremony_id())
            .await
            .is_completed(&definition));
    }

    #[tokio::test]
    async fn unsatisfied_guard_is_rejected() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let usecase = ApplyCeremonyTransitionUseCase::new(
            definition_resolver(definitions),
            stream(instances),
            Arc::new(FixedClock::new(now())),
        );

        let err = usecase
            .execute(ApplyCeremonyTransitionInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                trigger(),
            ))
            .await
            .unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
    }

    /// A session ready to make its last move.
    async fn ready_to_finish() -> (Arc<DefinitionRepositoryFake>, Arc<EventStoreFake>) {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let mut instance = started_instance(&definition);
        instance
            .start_step(
                &definition,
                &step_id(),
                made_core::value_objects::StepLease::new(
                    crate::usecases::ceremony_test_support::lease_owner(),
                    crate::usecases::ceremony_test_support::idempotency_key("lease-1"),
                    now(),
                    now() + time::Duration::seconds(60),
                )
                .unwrap(),
                now(),
            )
            .unwrap();
        instance
            .apply_step_result(
                &definition,
                &step_id(),
                StepResult::completed(StepOutput::empty()).unwrap(),
                now(),
            )
            .unwrap();
        instances.save(&instance).await.unwrap();
        (definitions, instances)
    }

    /// The move and the end it reached are two facts, not one.
    ///
    /// A reader asking whether this session finished should find that
    /// answered outright. Left to be worked out from the state the last
    /// move landed in, the answer depends on holding the definition
    /// too, which the journal does not carry.
    #[tokio::test]
    async fn seals_the_move_and_the_end_it_reached() {
        let (definitions, instances) = ready_to_finish().await;
        let (stream, store) = stream_over(instances);
        let usecase = ApplyCeremonyTransitionUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(ApplyCeremonyTransitionInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                trigger(),
            ))
            .await
            .unwrap();

        let facts = store.facts().await;
        let sealed = facts
            .iter()
            .map(|fact| fact.event.event_type())
            .collect::<Vec<_>>();
        let CeremonyEvent::TransitionApplied(moved) = &facts[0].event else {
            panic!("a move seals the transition: {:?}", facts[0].event);
        };
        assert_eq!(moved.transition.trigger(), &trigger());
        assert_eq!(moved.transition.applied_by(), Some(&role_id()));
        assert_eq!(
            sealed,
            vec![
                AuditEventType::TransitionApplied,
                AuditEventType::CeremonyCompleted
            ],
            "the move and the ending must both be sealed: {facts:?}"
        );
        // Declared by the caller, carried through untouched. The seat
        // came from the definition; what filled it did not.
        assert!(facts
            .iter()
            .all(|fact| fact.actor.kind() == AuditActorKind::Agent));
    }

    /// A move that was refused leaves nothing behind.
    ///
    /// The whole point of committing state and facts together: a
    /// rejected transition must not seal a fact saying it happened, and
    /// a sealed fact must not survive a transition that did not.
    #[tokio::test]
    async fn a_refused_move_seals_nothing() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let (stream, store) = stream_over(instances);
        let usecase = ApplyCeremonyTransitionUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(ApplyCeremonyTransitionInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                trigger(),
            ))
            .await
            .unwrap_err();

        assert!(
            store.facts().await.is_empty(),
            "an unsatisfied guard sealed a fact"
        );
    }

    /// The same race the guards refuse, refused here too.
    #[tokio::test]
    async fn refuses_to_move_a_session_someone_else_moved_on() {
        let (definitions, instances) = ready_to_finish().await;
        let usecase = ApplyCeremonyTransitionUseCase::new(
            definition_resolver(definitions),
            stream_losing_every_race(instances),
            Arc::new(FixedClock::new(now())),
        );

        let refused = usecase
            .execute(ApplyCeremonyTransitionInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                trigger(),
            ))
            .await;

        assert!(
            matches!(
                refused,
                Err(DomainError::Conflict {
                    what: "ceremony_instance"
                })
            ),
            "a lost race must be reported as one, got {refused:?}"
        );
    }

    /// A move fails fast: one conflict is the answer, not a reason to
    /// decide again.
    ///
    /// The store here refuses exactly one append and would take the
    /// next. A retrying command lands on its second attempt against
    /// it; a transition must not, because the state it was chosen
    /// against is gone and the caller — not the engine — decides what
    /// to do about that. Nothing lands.
    #[tokio::test]
    async fn a_move_that_lost_one_race_is_not_decided_again() {
        let (definitions, instances) = ready_to_finish().await;
        let usecase = ApplyCeremonyTransitionUseCase::new(
            definition_resolver(definitions),
            stream_conflicting_once(instances.clone()),
            Arc::new(FixedClock::new(now())),
        );

        let refused = usecase
            .execute(ApplyCeremonyTransitionInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                trigger(),
            ))
            .await;

        assert!(
            matches!(
                refused,
                Err(DomainError::Conflict {
                    what: "ceremony_instance"
                })
            ),
            "a transition fails fast on a conflict, got {refused:?}"
        );
        assert!(
            instances.facts().await.is_empty(),
            "a refused move must leave nothing in the stream"
        );
        assert!(
            instances
                .saved(&ceremony_id())
                .await
                .transitions()
                .is_empty(),
            "a refused move must not have moved the session"
        );
    }

    /// The assumption `session_facts` rests on.
    ///
    /// `AuditEventType::CeremonyFailed` has no producer, because a
    /// session reaches a terminal state only by moving into one and
    /// that always stamps it completed. This test is where that stops
    /// being true: an ending that is not a completion breaks it, and
    /// whoever adds one is then pointed at the two places — here and
    /// `session_memory_projection::ending_entry` — that already have a
    /// branch waiting for it.
    #[tokio::test]
    async fn a_terminal_session_is_always_a_finished_one() {
        let (definitions, instances) = ready_to_finish().await;
        let definition = definition();
        let usecase = ApplyCeremonyTransitionUseCase::new(
            definition_resolver(definitions),
            stream(instances),
            Arc::new(FixedClock::new(now())),
        );

        let ended = usecase
            .execute(ApplyCeremonyTransitionInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                trigger(),
            ))
            .await
            .unwrap();

        assert!(ended.is_terminal(&definition));
        assert!(
            ended.is_completed(&definition),
            "a terminal session that is not completed now exists, and the audit cannot say so"
        );
    }

    /// What a later session is told about how this one ended.
    ///
    /// Nothing asserted this before, which is how a branch describing
    /// an ending that cannot happen sat here unnoticed: memory only
    /// ever writes one kind of ending, and now something says so.
    #[tokio::test]
    async fn remembers_the_ending_as_the_only_kind_there_is() {
        let (definitions, instances) = ready_to_finish().await;
        let memory = recording_memory();
        let usecase = ApplyCeremonyTransitionUseCase::new(
            definition_resolver(definitions),
            remembering_stream(instances, memory.clone()),
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(ApplyCeremonyTransitionInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                trigger(),
            ))
            .await
            .unwrap();

        let summaries = memory
            .entries()
            .await
            .iter()
            .map(|entry| entry.summary().to_owned())
            .collect::<Vec<_>>();
        assert!(
            summaries
                .iter()
                .any(|summary| summary == "the session finished in `COMPLETED`"),
            "the ending did not reach memory: {summaries:?}"
        );
        assert!(
            !summaries
                .iter()
                .any(|summary| summary.contains("without finishing")),
            "memory claimed an ending the engine cannot tell apart: {summaries:?}"
        );
    }
}
