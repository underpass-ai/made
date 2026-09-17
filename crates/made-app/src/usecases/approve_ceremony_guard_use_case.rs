//! [`ApproveCeremonyGuardUseCase`] — record a human guard approval.

use std::sync::Arc;

use made_core::entities::ceremony_commands::ApproveGuard;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::approve_ceremony_guard_input::ApproveCeremonyGuardInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct ApproveCeremonyGuardUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for ApproveCeremonyGuardUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApproveCeremonyGuardUseCase").finish()
    }
}

impl ApproveCeremonyGuardUseCase {
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
        name = "approve_ceremony_guard",
        skip_all,
        fields(
            ceremony_id = %input.instance_id,
            guard_name = %input.guard_name,
            role_id = %input.role_id,
        )
    )]
    pub async fn execute(
        &self,
        input: ApproveCeremonyGuardInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::seat(&input.role_id, input.role_kind)?;
        let now = self.clock.now();
        let command = CeremonyCommand::ApproveGuard(ApproveGuard {
            guard_name: input.guard_name,
            approved_by: input.role_id,
            approved_by_kind: input.role_kind,
            now,
        });
        // Decided against the fold and appended where it was decided.
        // An approval commutes with what other writers do to the
        // session, so a lost race is decided again rather than refused.
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
    use made_core::entities::CeremonyEvent;
    use std::sync::Arc;

    use made_core::value_objects::GuardName;

    use made_core::error::DomainError;
    use made_core::value_objects::{AuditActorKind, AuditEventType};

    use super::*;
    use crate::usecases::ceremony_test_support::{
        approval_definition, ceremony_id, definition_resolver, now, role_id, started_instance,
        stream, stream_losing_every_race_over, stream_over, DefinitionRepositoryFake,
        EventStoreFake, FixedClock,
    };

    #[tokio::test]
    async fn records_human_guard_approval_in_context() {
        let definition = approval_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let usecase = ApproveCeremonyGuardUseCase::new(
            definition_resolver(definitions),
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
        );
        let guard_name = GuardName::new("human_approved").unwrap();

        let approved = usecase
            .execute(ApproveCeremonyGuardInput::new(
                ceremony_id(),
                guard_name.clone(),
                role_id(),
                AuditActorKind::Human,
            ))
            .await
            .unwrap();

        assert!(approved.context().is_guard_approved(&guard_name));
        assert!(instances
            .saved(&ceremony_id())
            .await
            .context()
            .is_guard_approved(&guard_name));
    }

    /// An approval that lost a race must say so, not win it quietly.
    ///
    /// The failure being excluded is `Ok`. A silent overwrite returns
    /// the approved session and looks like success to everyone, so the
    /// only observable difference between the safe implementation and
    /// the dangerous one is whether this call is refused.
    ///
    /// An approval commutes with most writes, so it is decided again
    /// on a conflict — but only up to its bound. Against a stream that
    /// moves under every attempt, the use case tries exactly as many
    /// times as the default retry allows and then reports the race.
    #[tokio::test]
    async fn refuses_to_approve_over_a_session_someone_else_moved_on() {
        let definition = approval_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let (stream, losing) = stream_losing_every_race_over(instances.clone());
        let usecase = ApproveCeremonyGuardUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        let refused = usecase
            .execute(ApproveCeremonyGuardInput::new(
                ceremony_id(),
                GuardName::new("human_approved").unwrap(),
                role_id(),
                AuditActorKind::Human,
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
        assert_eq!(
            losing.appends(),
            usize::from(ConflictPolicy::retry().attempts()),
            "a retrying command tries exactly its bound before giving up"
        );
        assert!(
            instances.facts().await.is_empty(),
            "a refused approval must leave nothing in the stream"
        );
    }

    /// The approval and the record of it land together.
    ///
    /// A guard that opens without leaving a fact behind is the failure
    /// the append exists to prevent, and it is invisible from the
    /// state alone — the session looks exactly the same either way.
    #[tokio::test]
    async fn seals_the_approval_into_the_journal() {
        let definition = approval_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let (stream, store) = stream_over(instances.clone());
        let usecase = ApproveCeremonyGuardUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(ApproveCeremonyGuardInput::new(
                ceremony_id(),
                GuardName::new("human_approved").unwrap(),
                role_id(),
                AuditActorKind::Human,
            ))
            .await
            .unwrap();

        let facts = store.facts().await;
        assert_eq!(facts.len(), 1, "one approval, one fact: {facts:?}");
        assert_eq!(
            facts[0].event.event_type(),
            AuditEventType::HumanApprovalRecorded
        );
        let CeremonyEvent::HumanApprovalRecorded(approved) = &facts[0].event else {
            panic!(
                "an approval seals who let the guard through: {:?}",
                facts[0].event
            );
        };
        assert_eq!(approved.approval.guard_name().as_str(), "human_approved");
        assert_eq!(approved.approval.approved_by(), &role_id());
        assert_eq!(facts[0].actor.kind(), AuditActorKind::Human);
    }
}
