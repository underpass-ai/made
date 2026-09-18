use std::sync::Arc;

use made_core::entities::ceremony_commands::AcceptChildCompletion;
use made_core::entities::{CeremonyCommand, CeremonyEvent, PublishedCeremonyDefinition};
use made_core::error::DomainError;
use made_core::ports::{CeremonyDefinitionPublicationPort, ClockPort};
use made_core::value_objects::{AuditActorKind, ChildCompletionRef, PlannedChild};

use super::{
    AcceptChildCompletionInput, AcceptChildCompletionOutput, ResolveCeremonyDefinitionUseCase,
};
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct AcceptChildCompletionUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl AcceptChildCompletionUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        publications: Arc<dyn CeremonyDefinitionPublicationPort>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            publications,
            stream,
            clock,
        }
    }

    pub async fn execute(
        &self,
        input: AcceptChildCompletionInput,
    ) -> Result<AcceptChildCompletionOutput, DomainError> {
        let child_records = self.stream.records(&input.child_id).await?;
        verify_chain(&child_records)?;
        let terminal = child_records
            .iter()
            .find(|record| record.event_id() == &input.terminal_event_id)
            .ok_or(DomainError::NotFound {
                what: "child_ceremony_terminal",
            })?;
        if !matches!(terminal.event(), Some(CeremonyEvent::CeremonyCompleted(_))) {
            return Err(DomainError::InvariantViolated {
                reason: "the located child record is not its completed terminal",
            });
        }
        let child = SessionStream::fold_records(&child_records)?.instance;
        let child_definition = self.definitions.execute(&child).await?;
        if !child.is_completed(&child_definition) {
            return Err(DomainError::InvariantViolated {
                reason: "the child journal does not fold to a terminal ceremony",
            });
        }
        let lineage = child
            .lineage()
            .cloned()
            .ok_or(DomainError::InvariantViolated {
                reason: "a child completion requires sealed ceremony lineage",
            })?;
        if child.id() != &input.child_id {
            return Err(DomainError::InvariantViolated {
                reason: "the child journal identity differs from its locator",
            });
        }

        let parent = self.stream.load(lineage.parent_id()).await?;
        let definition = self.definitions.execute(&parent.instance).await?;
        let group =
            parent
                .instance
                .child_group(lineage.group_id())
                .ok_or(DomainError::NotFound {
                    what: "child_spawn_group",
                })?;
        let planned = group
            .plan()
            .children()
            .iter()
            .find(|planned| planned.child_id() == &input.child_id)
            .ok_or(DomainError::InvariantViolated {
                reason: "the child is not present in its parent's sealed spawn plan",
            })?;
        self.verify_planned_child(planned, &child_records).await?;

        let completion = ChildCompletionRef::new(
            lineage.group_id().clone(),
            input.child_id,
            input.terminal_event_id,
            terminal.record_hash(),
        );
        let accepted_at = self.clock.now();
        let command = CeremonyCommand::AcceptChildCompletion(AcceptChildCompletion {
            completion: completion.clone(),
            now: accepted_at,
        });
        let actor = session_facts::party("made-child-completion", AuditActorKind::Engine)?;
        let accepted = self
            .stream
            .execute(parent, ConflictPolicy::retry(), |current| {
                let events = current.instance.decide(&command, &definition)?;
                session_facts::facts(&current.instance, events, &actor, accepted_at)
            })
            .await?;
        Ok(AcceptChildCompletionOutput::new(
            accepted.instance,
            completion,
        ))
    }

    async fn verify_planned_child(
        &self,
        planned: &PlannedChild,
        records: &[made_core::entities::AuditRecord],
    ) -> Result<(), DomainError> {
        let published = self.publication(planned).await?;
        if published.digest() != planned.digest() {
            return Err(DomainError::InvariantViolated {
                reason: "sealed child publication digest changed",
            });
        }
        let expected = made_core::entities::CeremonyInstance::decide_start_bound_child(
            planned.child_id().clone(),
            &published,
            planned.context().clone(),
            planned.lineage().clone(),
            planned.recollection().cloned(),
            planned.opened_at(),
        )?;
        if records.len() < expected.len()
            || records
                .iter()
                .zip(expected.iter())
                .any(|(record, event)| record.event() != Some(event))
        {
            return Err(DomainError::InvariantViolated {
                reason: "child opening differs from its parent's sealed spawn plan",
            });
        }
        Ok(())
    }

    async fn publication(
        &self,
        planned: &PlannedChild,
    ) -> Result<PublishedCeremonyDefinition, DomainError> {
        self.publications
            .published(planned.ceremony(), planned.version())
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_ceremony_definition",
            })
    }
}

impl std::fmt::Debug for AcceptChildCompletionUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AcceptChildCompletionUseCase")
            .finish()
    }
}

fn verify_chain(records: &[made_core::entities::AuditRecord]) -> Result<(), DomainError> {
    if !made_core::entities::AuditChain::verify(records).is_intact() {
        return Err(DomainError::InvariantViolated {
            reason: "child ceremony journal is not intact",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use made_core::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
    use made_core::ports::{CeremonyDefinitionRepositoryPort, CeremonySnapshotStorePort};
    use made_core::value_objects::{
        AuditActorKind, CeremonyChildSpawn, CeremonyChildSpec, CeremonyContext, CeremonyGuard,
        CeremonyId, CeremonyName, CeremonyRole, CeremonyState, CeremonyStep, CeremonyTransition,
        CeremonyVersion, ChildJoin, ChildrenCompletedCondition, GuardCondition, GuardName,
        IdempotencyKey, LeaseOwnerId, MaxChildDepth, MaxChildren, RetryPolicy, RoleAction, RoleId,
        StateId, StepHandlerConfig, StepHandlerKind, StepId, StepOutput, StepResult,
        TransitionTrigger,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        a_memory, lease_ttl, now, resolver_with, stream, DefinitionRepositoryFake, EventStoreFake,
        FixedClock, PublicationsFake, StepHandlerFake,
    };
    use crate::usecases::{
        ApplyCeremonyTransitionInput, ApplyCeremonyTransitionUseCase,
        PrepareCeremonyChildrenUseCase, RunCeremonyStepInput, RunCeremonyStepUseCase,
    };

    fn role_id() -> RoleId {
        RoleId::new("DRIVER").unwrap()
    }

    fn spawn_definition(
        name: &str,
        child_name: &str,
        child_count: usize,
        join: ChildJoin,
    ) -> CeremonyDefinition {
        let work = StateId::new("WORK").unwrap();
        let done = StateId::new("DONE").unwrap();
        let step_id = StepId::new("spawn_children").unwrap();
        let children = (0..child_count)
            .map(|_| {
                CeremonyChildSpec::new(
                    CeremonyName::new(child_name).unwrap(),
                    CeremonyVersion::v1(),
                    BTreeMap::new(),
                )
            })
            .collect::<Vec<_>>();
        let step = CeremonyStep::new(
            step_id.clone(),
            work.clone(),
            StepHandlerKind::new("must_not_run").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        )
        .with_spawn(
            CeremonyChildSpawn::new(
                children,
                MaxChildren::new(u16::try_from(child_count).unwrap()).unwrap(),
                MaxChildDepth::new(3).unwrap(),
            )
            .unwrap(),
        );
        let guard = CeremonyGuard::new(
            GuardName::new("children_ready").unwrap(),
            GuardCondition::ChildrenCompleted(ChildrenCompletedCondition::new(
                step_id.clone(),
                join,
            )),
        );
        let transition = CeremonyTransition::new(
            work.clone(),
            done.clone(),
            TransitionTrigger::new("finish").unwrap(),
            vec![guard.name().clone()],
        )
        .unwrap();
        let role = CeremonyRole::new(
            role_id(),
            vec![
                RoleAction::step(step_id),
                RoleAction::transition(transition.trigger().clone()),
            ],
        )
        .unwrap();
        CeremonyDefinition::new(
            CeremonyName::new(name).unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![CeremonyState::initial(work), CeremonyState::terminal(done)],
            vec![transition],
            vec![step],
            vec![guard],
            vec![role],
        )
        .unwrap()
    }

    fn leaf_definition() -> CeremonyDefinition {
        let work = StateId::new("WORK").unwrap();
        let done = StateId::new("DONE").unwrap();
        let step_id = StepId::new("work").unwrap();
        let step = CeremonyStep::new(
            step_id.clone(),
            work.clone(),
            StepHandlerKind::new("controlled").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        );
        let guard = CeremonyGuard::new(
            GuardName::new("work_done").unwrap(),
            GuardCondition::StepStatus {
                step_id: step_id.clone(),
                status: made_core::value_objects::StepStatus::Completed,
            },
        );
        let transition = CeremonyTransition::new(
            work.clone(),
            done.clone(),
            TransitionTrigger::new("finish").unwrap(),
            vec![guard.name().clone()],
        )
        .unwrap();
        let role = CeremonyRole::new(
            role_id(),
            vec![
                RoleAction::step(step_id),
                RoleAction::transition(transition.trigger().clone()),
            ],
        )
        .unwrap();
        CeremonyDefinition::new(
            CeremonyName::new("leaf_child").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![CeremonyState::initial(work), CeremonyState::terminal(done)],
            vec![transition],
            vec![step],
            vec![guard],
            vec![role],
        )
        .unwrap()
    }

    fn terminal_event_id(
        records: &[made_core::entities::AuditRecord],
    ) -> made_core::value_objects::EventId {
        records
            .iter()
            .find(|record| matches!(record.event(), Some(CeremonyEvent::CeremonyCompleted(_))))
            .unwrap()
            .event_id()
            .clone()
    }

    #[tokio::test]
    async fn nested_join_any_accepts_a_late_sibling_and_replays_the_terminal_locator() {
        let root_definition = spawn_definition("root_parent", "middle_child", 1, ChildJoin::All);
        let middle_definition = spawn_definition("middle_child", "leaf_child", 2, ChildJoin::Any);
        let leaf_definition = leaf_definition();
        let root_id = CeremonyId::new("root-parent").unwrap();
        let definitions = Arc::new(DefinitionRepositoryFake::new(root_definition.clone()));
        definitions.save(&middle_definition).await.unwrap();
        definitions.save(&leaf_definition).await.unwrap();
        let publications = Arc::new(PublicationsFake::default());
        publications.seed(middle_definition.clone()).await;
        publications.seed(leaf_definition.clone()).await;
        let store = Arc::new(EventStoreFake::default());
        let root = CeremonyInstance::start(
            root_id.clone(),
            &root_definition,
            CeremonyContext::empty(),
            now(),
        )
        .unwrap();
        store.save(&root).await.unwrap();
        let session_stream = stream(store.clone());
        let resolver = resolver_with(definitions, publications.clone());
        let prepare = Arc::new(PrepareCeremonyChildrenUseCase::new(
            resolver.clone(),
            publications.clone(),
            session_stream.clone(),
            Arc::new(FixedClock::new(now())),
            a_memory(),
        ));
        let runner = RunCeremonyStepUseCase::new(
            resolver.clone(),
            session_stream.clone(),
            Arc::new(StepHandlerFake::succeeding(
                StepResult::completed(StepOutput::empty()).unwrap(),
            )),
            Arc::new(FixedClock::new(now())),
        )
        .with_child_orchestrator(prepare);

        let root_spawn = runner
            .execute(RunCeremonyStepInput::new(
                root_id.clone(),
                role_id(),
                AuditActorKind::Service,
                StepId::new("spawn_children").unwrap(),
                LeaseOwnerId::new("root-runner").unwrap(),
                IdempotencyKey::new("root-spawn").unwrap(),
                lease_ttl(),
            ))
            .await
            .unwrap();
        let middle_id = root_spawn
            .instance()
            .child_groups()
            .values()
            .next()
            .unwrap()
            .plan()
            .children()[0]
            .child_id()
            .clone();
        let middle_spawn = runner
            .execute(RunCeremonyStepInput::new(
                middle_id.clone(),
                role_id(),
                AuditActorKind::Service,
                StepId::new("spawn_children").unwrap(),
                LeaseOwnerId::new("middle-runner").unwrap(),
                IdempotencyKey::new("middle-spawn").unwrap(),
                lease_ttl(),
            ))
            .await
            .unwrap();
        let leaf_ids = middle_spawn
            .instance()
            .child_groups()
            .values()
            .next()
            .unwrap()
            .plan()
            .children()
            .iter()
            .map(|child| child.child_id().clone())
            .collect::<Vec<_>>();
        let transitions = ApplyCeremonyTransitionUseCase::new(
            resolver.clone(),
            session_stream.clone(),
            Arc::new(FixedClock::new(now())),
        );
        let accept = AcceptChildCompletionUseCase::new(
            resolver.clone(),
            publications.clone(),
            session_stream.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let mut leaf_terminals = Vec::new();
        for (index, leaf_id) in leaf_ids.iter().enumerate() {
            runner
                .execute(RunCeremonyStepInput::new(
                    leaf_id.clone(),
                    role_id(),
                    AuditActorKind::Agent,
                    StepId::new("work").unwrap(),
                    LeaseOwnerId::new(format!("leaf-runner-{index}")).unwrap(),
                    IdempotencyKey::new(format!("leaf-work-{index}")).unwrap(),
                    lease_ttl(),
                ))
                .await
                .unwrap();
            transitions
                .execute(ApplyCeremonyTransitionInput::new(
                    leaf_id.clone(),
                    role_id(),
                    AuditActorKind::Agent,
                    TransitionTrigger::new("finish").unwrap(),
                ))
                .await
                .unwrap();
            leaf_terminals.push(terminal_event_id(&store.records(leaf_id).await));
        }

        accept
            .execute(AcceptChildCompletionInput::new(
                leaf_ids[0].clone(),
                leaf_terminals[0].clone(),
            ))
            .await
            .unwrap();
        transitions
            .execute(ApplyCeremonyTransitionInput::new(
                middle_id.clone(),
                role_id(),
                AuditActorKind::Service,
                TransitionTrigger::new("finish").unwrap(),
            ))
            .await
            .unwrap();
        let middle_terminal = terminal_event_id(&store.records(&middle_id).await);
        accept
            .execute(AcceptChildCompletionInput::new(
                leaf_ids[1].clone(),
                leaf_terminals[1].clone(),
            ))
            .await
            .unwrap();
        let middle_records = store.records(&middle_id).await;
        let terminal_index = middle_records
            .iter()
            .position(|record| record.event_id() == &middle_terminal)
            .unwrap();
        assert!(matches!(
            middle_records.last().unwrap().event(),
            Some(CeremonyEvent::ChildCompletionAccepted(_))
        ));
        assert!(terminal_index < middle_records.len() - 1);

        store.forget(&middle_id).await.unwrap();
        store.forget(&root_id).await.unwrap();
        let restarted_stream = stream(store.clone());
        assert!(restarted_stream
            .load(&middle_id)
            .await
            .unwrap()
            .instance
            .is_completed(&middle_definition));
        let restarted_accept = AcceptChildCompletionUseCase::new(
            resolver,
            publications,
            restarted_stream,
            Arc::new(FixedClock::new(now())),
        );
        for _ in 0..2 {
            restarted_accept
                .execute(AcceptChildCompletionInput::new(
                    middle_id.clone(),
                    middle_terminal.clone(),
                ))
                .await
                .unwrap();
        }
        let root_records = store.records(&root_id).await;
        assert_eq!(
            root_records
                .iter()
                .filter(|record| matches!(
                    record.event(),
                    Some(CeremonyEvent::ChildCompletionAccepted(_))
                ))
                .count(),
            1,
            "replaying the same nested terminal appended a duplicate acceptance"
        );
    }
}
