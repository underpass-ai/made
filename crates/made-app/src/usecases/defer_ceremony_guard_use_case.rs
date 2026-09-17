//! [`DeferCeremonyGuardUseCase`] — preserve a human decision deferral.

use std::sync::Arc;

use made_core::entities::ceremony_commands::DeferGuard;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::defer_ceremony_guard_input::DeferCeremonyGuardInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct DeferCeremonyGuardUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for DeferCeremonyGuardUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeferCeremonyGuardUseCase").finish()
    }
}

impl DeferCeremonyGuardUseCase {
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
        name = "defer_ceremony_guard",
        skip_all,
        fields(ceremony_id = %input.instance_id, guard_name = %input.guard_name)
    )]
    pub async fn execute(
        &self,
        input: DeferCeremonyGuardInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::seat(&input.role_id, input.role_kind)?;
        let now = self.clock.now();
        let command = CeremonyCommand::DeferGuard(DeferGuard {
            guard_name: input.guard_name,
            content: input.content,
            deferred_by: input.role_id,
            deferred_by_kind: input.role_kind,
            now,
        });
        // Deciding not to decide is a decision, and it lands for the
        // same reason an approval does: decided against the fold,
        // decided again if another writer got there first.
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
    use made_core::value_objects::{CeremonyGuardDeferralContent, GuardName};

    use made_core::value_objects::{AuditActorKind, AuditEventType};

    use super::*;
    use crate::usecases::ceremony_test_support::{
        approval_definition, ceremony_id, definition_resolver, now, role_id, started_instance,
        stream, stream_over, DefinitionRepositoryFake, EventStoreFake, FixedClock,
    };

    #[tokio::test]
    async fn persists_a_human_guard_deferral_without_satisfying_the_guard() {
        let definition = approval_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let usecase = DeferCeremonyGuardUseCase::new(
            definition_resolver(definitions),
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
        );
        let guard_name = GuardName::new("human_approved").unwrap();

        let deferred = usecase
            .execute(DeferCeremonyGuardInput::new(
                ceremony_id(),
                guard_name.clone(),
                CeremonyGuardDeferralContent::new(
                    "I do not know.",
                    "The available evidence is inconclusive.",
                    vec!["New evidence clarifies the outcome.".to_owned()],
                )
                .unwrap(),
                role_id(),
                AuditActorKind::Human,
            ))
            .await
            .unwrap();

        assert!(!deferred.context().is_guard_approved(&guard_name));
        assert_eq!(deferred.guard_deferrals().len(), 1);
        assert_eq!(
            instances
                .saved(&ceremony_id())
                .await
                .guard_deferrals()
                .len(),
            1
        );
    }

    /// A deferral is a decision, and it leaves the same kind of trace.
    ///
    /// The guard stays unsatisfied either way, so the state cannot tell
    /// a session that was deliberately left open from one nobody ever
    /// looked at. The journal is the only place that difference exists.
    #[tokio::test]
    async fn seals_the_deferral_into_the_journal() {
        let definition = approval_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let (stream, store) = stream_over(instances.clone());
        let usecase = DeferCeremonyGuardUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(DeferCeremonyGuardInput::new(
                ceremony_id(),
                GuardName::new("human_approved").unwrap(),
                CeremonyGuardDeferralContent::new(
                    "I do not know.",
                    "The available evidence is inconclusive.",
                    vec!["New evidence clarifies the outcome.".to_owned()],
                )
                .unwrap(),
                role_id(),
                AuditActorKind::Human,
            ))
            .await
            .unwrap();

        let facts = store.facts().await;
        assert_eq!(facts.len(), 1, "one deferral, one fact: {facts:?}");
        assert_eq!(
            facts[0].event.event_type(),
            AuditEventType::HumanDeferralRecorded
        );
    }
}
