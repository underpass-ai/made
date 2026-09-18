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
        let terminal = child_records.last().ok_or(DomainError::NotFound {
            what: "child_ceremony_terminal",
        })?;
        if terminal.event_id() != &input.terminal_event_id
            || !matches!(terminal.event(), Some(CeremonyEvent::CeremonyCompleted(_)))
        {
            return Err(DomainError::InvariantViolated {
                reason: "the located child record is not its completed terminal",
            });
        }
        let child = SessionStream::fold_records(&child_records)?.instance;
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
