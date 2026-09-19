use std::sync::Arc;

use made_core::entities::ceremony_commands::{
    AdoptChildSpawnPlan, ApplyStepResult, PlanCeremonyChildren,
};
use made_core::entities::{
    CeremonyCommand, CeremonyEvent, CeremonyInstance, PublishedCeremonyDefinition,
};
use made_core::error::DomainError;
use made_core::ports::{CeremonyDefinitionPublicationPort, ClockPort, MemoryReaderPort};
use made_core::value_objects::{
    AuditActorKind, CeremonyId, CeremonyLineage, ChildCeremonyId, ChildDepth, ChildDepthBudget,
    ChildGroupId, ChildPosition, ChildSpawnCoordinates, ChildSpawnPlan, PlannedChild, StepStatus,
};

mod support;
#[cfg(test)]
mod tests;

use support::{
    completed_spawn_is_same, project_context, same_spawn_request_is_sealed, spawn_result,
    verify_chain,
};

use super::{
    PrepareCeremonyChildrenInput, PrepareCeremonyChildrenOutput, ResolveCeremonyDefinitionUseCase,
};
use crate::services::{
    memory_scope_resolver, session_facts, session_recall, ConflictPolicy, SessionStream,
};

pub struct PrepareCeremonyChildrenUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
    memory: Arc<dyn MemoryReaderPort>,
}

impl std::fmt::Debug for PrepareCeremonyChildrenUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrepareCeremonyChildrenUseCase")
            .finish()
    }
}

impl PrepareCeremonyChildrenUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        publications: Arc<dyn CeremonyDefinitionPublicationPort>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
        memory: Arc<dyn MemoryReaderPort>,
    ) -> Self {
        Self {
            definitions,
            publications,
            stream,
            clock,
            memory,
        }
    }

    pub async fn execute(
        &self,
        input: PrepareCeremonyChildrenInput,
    ) -> Result<PrepareCeremonyChildrenOutput, DomainError> {
        let loaded = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&loaded.instance).await?;
        let existing_plan = Self::current_plan(&loaded.instance, &input.step_id);
        if let Some(plan) = &existing_plan {
            if loaded
                .instance
                .step_record(&input.step_id)
                .is_some_and(|record| record.status() == StepStatus::Completed)
            {
                return self
                    .verify_completed_spawn(loaded.instance, plan.clone(), &input.step_id)
                    .await;
            }
        }
        let actor = Self::step_actor(&loaded.instance, &definition, &input)?;
        let planned_at = self.clock.now();
        let (planned, plan) = if let Some(plan) = existing_plan {
            (loaded, plan)
        } else {
            let candidate = self
                .build_plan(&loaded.instance, &definition, &input)
                .await?;
            let planned = self
                .stream
                .execute(loaded, ConflictPolicy::retry(), |session| {
                    if same_spawn_request_is_sealed(
                        &session.instance,
                        &candidate,
                        &input.step_id,
                        &input.claim_fence,
                    )? {
                        return Ok(Vec::new());
                    }
                    let command = CeremonyCommand::PlanCeremonyChildren(PlanCeremonyChildren {
                        plan: candidate.clone(),
                        now: planned_at,
                    });
                    let events = session.instance.decide(&command, &definition)?;
                    session_facts::facts(&session.instance, events, &actor, planned_at)
                })
                .await?;
            let plan = planned
                .instance
                .child_group(candidate.group_id())
                .ok_or(DomainError::NotFound {
                    what: "child_spawn_group",
                })?
                .plan()
                .clone();
            (planned, plan)
        };
        let adopted = self
            .adopt_if_needed(planned, &definition, &input, &plan, &actor)
            .await?;
        let sealed_plan = adopted
            .instance
            .child_group(plan.group_id())
            .ok_or(DomainError::NotFound {
                what: "child_spawn_group",
            })?
            .plan()
            .clone();
        for child in sealed_plan.children() {
            self.open_or_verify_child(child).await?;
        }
        self.complete_spawn_step(definition, input, sealed_plan)
            .await
    }

    /// Resume a sealed group without inventing a claim.
    ///
    /// Missing children are opened from the plan. The spawn step is completed
    /// only when the group's durable adoption fence still equals the current
    /// step claim; a reclaimed but unadopted group remains available to the
    /// claimant that owns the replacement fence.
    pub async fn recover_group(
        &self,
        parent_id: &CeremonyId,
        group_id: &ChildGroupId,
    ) -> Result<Option<PrepareCeremonyChildrenOutput>, DomainError> {
        let loaded = self.stream.load(parent_id).await?;
        let definition = self.definitions.execute(&loaded.instance).await?;
        let plan = loaded
            .instance
            .child_group(group_id)
            .ok_or(DomainError::NotFound {
                what: "child_spawn_group",
            })?
            .plan()
            .clone();
        for child in plan.children() {
            self.open_or_verify_child(child).await?;
        }

        let current = self.stream.load(parent_id).await?;
        let group = current
            .instance
            .child_group(group_id)
            .ok_or(DomainError::NotFound {
                what: "child_spawn_group",
            })?;
        let coordinates = group.plan().coordinates();
        let step_id = coordinates.step_id().clone();
        let adopted_claim_fence = group.adopted_claim_fence().clone();
        let Some(record) = current.instance.step_record(&step_id) else {
            return Ok(None);
        };
        let current_group = ChildGroupId::derive(
            current.instance.id(),
            &ChildSpawnCoordinates::new(
                step_id.clone(),
                current.instance.current_state_visit(),
                current.instance.current_state_iteration(),
                record.iteration(),
            ),
        );
        if &current_group != group_id {
            return Ok(None);
        }
        if record.status() == StepStatus::Completed {
            return self
                .verify_completed_spawn(current.instance, plan, &step_id)
                .await
                .map(Some);
        }
        if record.status() != StepStatus::InProgress {
            return Ok(None);
        }
        let current_fence = current.instance.step_claim_fence(&step_id)?;
        if adopted_claim_fence != current_fence {
            return Ok(None);
        }

        let actor = Self::step_actor_for_kind(
            &current.instance,
            &definition,
            &step_id,
            AuditActorKind::Engine,
        )?;
        let child_ids = plan
            .children()
            .iter()
            .map(|child| child.child_id().clone())
            .collect::<Vec<_>>();
        let result = spawn_result(group_id, &child_ids)?;
        let completed_at = self.clock.now();
        let completion_step_id = step_id.clone();
        let command = CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id,
            result: result.clone(),
            claim_fence: current_fence,
            now: completed_at,
        });
        let completed = self
            .stream
            .execute(current, ConflictPolicy::retry(), |session| {
                if completed_spawn_is_same(&session.instance, &plan, &completion_step_id, &result)?
                {
                    return Ok(Vec::new());
                }
                let events = session.instance.decide(&command, &definition)?;
                session_facts::facts(&session.instance, events, &actor, completed_at)
            })
            .await?;
        Ok(Some(PrepareCeremonyChildrenOutput::new(
            completed.instance,
            group_id.clone(),
            child_ids,
            result,
        )))
    }

    fn current_plan(
        instance: &CeremonyInstance,
        step_id: &made_core::value_objects::StepId,
    ) -> Option<ChildSpawnPlan> {
        let record = instance.step_record(step_id)?;
        let coordinates = ChildSpawnCoordinates::new(
            step_id.clone(),
            instance.current_state_visit(),
            instance.current_state_iteration(),
            record.iteration(),
        );
        let group_id = ChildGroupId::derive(instance.id(), &coordinates);
        instance
            .child_group(&group_id)
            .map(|group| group.plan().clone())
    }

    async fn build_plan(
        &self,
        parent: &CeremonyInstance,
        definition: &made_core::entities::CeremonyDefinition,
        input: &PrepareCeremonyChildrenInput,
    ) -> Result<ChildSpawnPlan, DomainError> {
        let step = definition
            .step(&input.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_step",
            })?;
        let spawn = step.spawn().ok_or(DomainError::InvariantViolated {
            reason: "prepare children requires a child-spawning step",
        })?;
        if parent.step_claim_fence(&input.step_id)? != input.claim_fence {
            return Err(DomainError::InvariantViolated {
                reason: "child spawn preparation requires the current claim fence",
            });
        }
        let record = parent
            .step_record(&input.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_step_record",
            })?;
        let coordinates = ChildSpawnCoordinates::new(
            input.step_id.clone(),
            parent.current_state_visit(),
            parent.current_state_iteration(),
            record.iteration(),
        );
        let group_id = ChildGroupId::derive(parent.id(), &coordinates);
        let inherited = parent.lineage().map_or_else(
            || ChildDepthBudget::from(made_core::value_objects::MaxChildDepth::SERVER_MAX),
            CeremonyLineage::remaining_depth,
        );
        let remaining = inherited.for_child(spawn.max_depth())?;
        let depth = parent
            .lineage()
            .map_or(Ok(ChildDepth::FIRST), |lineage| lineage.depth().next())?;
        let root_id = parent
            .lineage()
            .map_or_else(|| parent.id().clone(), |lineage| lineage.root_id().clone());
        let now = self.clock.now();
        let mut children = Vec::with_capacity(spawn.children().len());
        for (position, spec) in spawn.children().iter().enumerate() {
            let position = u16::try_from(position).map_err(|_| DomainError::InvariantViolated {
                reason: "child position exceeds its bounded representation",
            })?;
            let position = ChildPosition::new(position);
            let child_id = ChildCeremonyId::derive(&group_id, position)?.into_ceremony_id();
            let published = self.publication(spec.ceremony(), spec.version()).await?;
            let context = project_context(parent.context(), spec.inputs())?;
            let lineage = CeremonyLineage::new(
                root_id.clone(),
                parent.id().clone(),
                group_id.clone(),
                position,
                depth,
                remaining,
            )?;
            let scope = memory_scope_resolver::of_context(&context, &child_id)?;
            let recollection = session_recall::recall(self.memory.as_ref(), &scope).await;
            // Validate every required child input and opening invariant before the parent plan lands.
            child_opening(
                child_id.clone(),
                &published,
                context.clone(),
                lineage.clone(),
                parent.budget_account_id().cloned(),
                recollection.clone(),
                now,
            )?;
            children.push(
                PlannedChild::new(
                    child_id,
                    position,
                    spec.ceremony().clone(),
                    spec.version().clone(),
                    published.digest(),
                    context,
                    lineage,
                    recollection,
                    now,
                )
                .with_budget_account_id(parent.budget_account_id().cloned()),
            );
        }
        ChildSpawnPlan::new(
            group_id,
            coordinates,
            input.claim_fence.clone(),
            children,
            spawn.max_children(),
            spawn.max_depth(),
        )
    }

    async fn publication(
        &self,
        name: &made_core::value_objects::CeremonyName,
        version: &made_core::value_objects::CeremonyVersion,
    ) -> Result<PublishedCeremonyDefinition, DomainError> {
        self.publications
            .published(name, version)
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_ceremony_definition",
            })
    }

    fn step_actor(
        instance: &CeremonyInstance,
        definition: &made_core::entities::CeremonyDefinition,
        input: &PrepareCeremonyChildrenInput,
    ) -> Result<made_core::value_objects::AuditActor, DomainError> {
        Self::step_actor_for_kind(instance, definition, &input.step_id, input.actor_kind)
    }

    fn step_actor_for_kind(
        instance: &CeremonyInstance,
        definition: &made_core::entities::CeremonyDefinition,
        step_id: &made_core::value_objects::StepId,
        actor_kind: AuditActorKind,
    ) -> Result<made_core::value_objects::AuditActor, DomainError> {
        let record = instance.step_record(step_id).ok_or(DomainError::NotFound {
            what: "ceremony_step_record",
        })?;
        let role = record
            .claimed_role()
            .cloned()
            .map_or_else(|| definition.role_id_for_step(step_id), Ok)?;
        session_facts::seat(&role, actor_kind)
    }

    async fn adopt_if_needed(
        &self,
        session: crate::services::LoadedSession,
        definition: &made_core::entities::CeremonyDefinition,
        input: &PrepareCeremonyChildrenInput,
        plan: &ChildSpawnPlan,
        actor: &made_core::value_objects::AuditActor,
    ) -> Result<crate::services::LoadedSession, DomainError> {
        let coordinates = session
            .instance
            .step_record(&input.step_id)
            .map(|record| {
                ChildSpawnCoordinates::new(
                    input.step_id.clone(),
                    session.instance.current_state_visit(),
                    session.instance.current_state_iteration(),
                    record.iteration(),
                )
            })
            .ok_or(DomainError::NotFound {
                what: "ceremony_step_record",
            })?;
        let group_id = ChildGroupId::derive(session.instance.id(), &coordinates);
        let adopted_at = self.clock.now();
        let command = CeremonyCommand::AdoptChildSpawnPlan(AdoptChildSpawnPlan {
            group_id,
            claim_fence: input.claim_fence.clone(),
            now: adopted_at,
        });
        self.stream
            .execute(session, ConflictPolicy::retry(), |current| {
                let child_ids = plan
                    .children()
                    .iter()
                    .map(|child| child.child_id().clone())
                    .collect::<Vec<_>>();
                let result = spawn_result(plan.group_id(), &child_ids)?;
                if completed_spawn_is_same(&current.instance, plan, &input.step_id, &result)? {
                    return Ok(Vec::new());
                }
                let events = current.instance.decide(&command, definition)?;
                session_facts::facts(&current.instance, events, actor, adopted_at)
            })
            .await
    }

    async fn open_or_verify_child(&self, child: &PlannedChild) -> Result<(), DomainError> {
        let published = self.publication(child.ceremony(), child.version()).await?;
        if published.digest() != child.digest() {
            return Err(DomainError::InvariantViolated {
                reason: "sealed child publication digest changed",
            });
        }
        let expected = child_opening(
            child.child_id().clone(),
            &published,
            child.context().clone(),
            child.lineage().clone(),
            child.budget_account_id().cloned(),
            child.recollection().cloned(),
            child.opened_at(),
        )?;
        let actor = session_facts::party("made-child-spawner", AuditActorKind::Service)?;
        match self
            .stream
            .open(expected.clone(), actor, child.opened_at())
            .await
        {
            Ok(_) => Ok(()),
            Err(DomainError::AlreadyExists { .. }) => {
                self.verify_existing_opening(child.child_id(), &expected)
                    .await
            }
            Err(error) => Err(error),
        }
    }

    async fn verify_existing_opening(
        &self,
        child_id: &CeremonyId,
        expected: &[CeremonyEvent],
    ) -> Result<(), DomainError> {
        let records = self.stream.records(child_id).await?;
        verify_chain(&records)?;
        if records.len() < expected.len()
            || records
                .iter()
                .zip(expected)
                .any(|(record, event)| record.event() != Some(event))
        {
            return Err(DomainError::AlreadyExists {
                what: "foreign_child_ceremony",
            });
        }
        Ok(())
    }

    async fn complete_spawn_step(
        &self,
        definition: made_core::entities::CeremonyDefinition,
        input: PrepareCeremonyChildrenInput,
        plan: ChildSpawnPlan,
    ) -> Result<PrepareCeremonyChildrenOutput, DomainError> {
        let child_ids = plan
            .children()
            .iter()
            .map(|child| child.child_id().clone())
            .collect::<Vec<_>>();
        let result = spawn_result(plan.group_id(), &child_ids)?;
        let session = self.stream.load(&input.instance_id).await?;
        if completed_spawn_is_same(&session.instance, &plan, &input.step_id, &result)? {
            return Ok(PrepareCeremonyChildrenOutput::new(
                session.instance,
                plan.group_id().clone(),
                child_ids,
                result,
            ));
        }
        let actor = Self::step_actor(&session.instance, &definition, &input)?;
        let completed_at = self.clock.now();
        let completion_step_id = input.step_id.clone();
        let command = CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: input.step_id,
            result: result.clone(),
            claim_fence: input.claim_fence,
            now: completed_at,
        });
        let completed = self
            .stream
            .execute(session, ConflictPolicy::retry(), |current| {
                if completed_spawn_is_same(&current.instance, &plan, &completion_step_id, &result)?
                {
                    return Ok(Vec::new());
                }
                let events = current.instance.decide(&command, &definition)?;
                session_facts::facts(&current.instance, events, &actor, completed_at)
            })
            .await?;
        Ok(PrepareCeremonyChildrenOutput::new(
            completed.instance,
            plan.group_id().clone(),
            child_ids,
            result,
        ))
    }

    async fn verify_completed_spawn(
        &self,
        instance: CeremonyInstance,
        plan: ChildSpawnPlan,
        step_id: &made_core::value_objects::StepId,
    ) -> Result<PrepareCeremonyChildrenOutput, DomainError> {
        for child in plan.children() {
            self.open_or_verify_child(child).await?;
        }
        let child_ids = plan
            .children()
            .iter()
            .map(|child| child.child_id().clone())
            .collect::<Vec<_>>();
        let result = spawn_result(plan.group_id(), &child_ids)?;
        if !completed_spawn_is_same(&instance, &plan, step_id, &result)? {
            return Err(DomainError::InvariantViolated {
                reason: "completed child spawn is not the sealed plan result",
            });
        }
        Ok(PrepareCeremonyChildrenOutput::new(
            instance,
            plan.group_id().clone(),
            child_ids,
            result,
        ))
    }
}

fn child_opening(
    child_id: CeremonyId,
    published: &PublishedCeremonyDefinition,
    context: made_core::value_objects::CeremonyContext,
    lineage: CeremonyLineage,
    account_id: Option<made_core::value_objects::BudgetAccountId>,
    recollection: Option<made_core::value_objects::SessionRecollection>,
    opened_at: time::OffsetDateTime,
) -> Result<Vec<CeremonyEvent>, DomainError> {
    match account_id {
        Some(account_id) => CeremonyInstance::decide_start_bound_budgeted_child(
            child_id,
            published,
            context,
            lineage,
            account_id,
            recollection,
            opened_at,
        ),
        None => CeremonyInstance::decide_start_bound_child(
            child_id,
            published,
            context,
            lineage,
            recollection,
            opened_at,
        ),
    }
}
