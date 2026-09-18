use crate::entities::ceremony_commands::{
    AcceptChildCompletion, AdoptChildSpawnPlan, PlanCeremonyChildren,
};
use crate::entities::ceremony_events::{
    ChildCompletionAccepted, ChildSpawnPlanAdopted, ChildSpawnPlanned,
};
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;

impl CeremonyInstance {
    pub(super) fn decide_plan_children(
        &self,
        command: &PlanCeremonyChildren,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        let plan = &command.plan;
        if let Some(existing) = self.child_groups.get(plan.group_id()) {
            return if existing.plan() == plan {
                Ok(Vec::new())
            } else {
                Err(DomainError::AlreadyExists {
                    what: "child_spawn_group",
                })
            };
        }
        self.require_admits_new_work("plan_children")?;
        let coordinates = plan.coordinates();
        if coordinates.state_visit() != self.current_state_visit
            || coordinates.state_iteration() != self.current_state_iteration
        {
            return Err(DomainError::InvariantViolated {
                reason: "child spawn plan coordinates are not current",
            });
        }
        let step = definition
            .step(coordinates.step_id())
            .ok_or(DomainError::NotFound {
                what: "ceremony_child_spawn.step",
            })?;
        if step.state_id() != &self.current_state {
            return Err(DomainError::InvariantViolated {
                reason: "child-spawning step does not belong to the current state",
            });
        }
        let spawn = step.spawn().ok_or(DomainError::InvariantViolated {
            reason: "child spawn plan requires a child-spawning step",
        })?;
        let record = self
            .step_record(coordinates.step_id())
            .ok_or(DomainError::NotFound {
                what: "ceremony_child_spawn.step_record",
            })?;
        if record.state_visit() != coordinates.state_visit()
            || record.state_iteration() != coordinates.state_iteration()
            || record.iteration() != coordinates.step_iteration()
        {
            return Err(DomainError::InvariantViolated {
                reason: "child spawn plan step iteration is not current",
            });
        }
        if plan.group_id() != &crate::value_objects::ChildGroupId::derive(self.id(), coordinates) {
            return Err(DomainError::InvariantViolated {
                reason: "child spawn group identity does not match parent and coordinates",
            });
        }
        if spawn.children().len() != plan.children().len()
            || spawn.max_children() != plan.max_children()
            || spawn.max_depth() != plan.max_depth()
        {
            return Err(DomainError::InvariantViolated {
                reason: "child spawn plan differs from the ceremony definition",
            });
        }
        let inherited_budget = self.lineage.as_ref().map_or_else(
            || {
                crate::value_objects::ChildDepthBudget::from(
                    crate::value_objects::MaxChildDepth::SERVER_MAX,
                )
            },
            crate::value_objects::CeremonyLineage::remaining_depth,
        );
        let expected_remaining = inherited_budget.for_child(spawn.max_depth())?;
        let expected_depth = self
            .lineage
            .as_ref()
            .map_or(Ok(crate::value_objects::ChildDepth::FIRST), |lineage| {
                lineage.depth().next()
            })?;
        let expected_root = self
            .lineage
            .as_ref()
            .map_or_else(|| self.id(), |lineage| lineage.root_id());
        for (child, spec) in plan.children().iter().zip(spawn.children()) {
            if child.ceremony() != spec.ceremony() || child.version() != spec.version() {
                return Err(DomainError::InvariantViolated {
                    reason: "planned child binding differs from the ceremony definition",
                });
            }
            if child.lineage().parent_id() != self.id()
                || child.lineage().group_id() != plan.group_id()
                || child.lineage().position() != child.position()
                || child.lineage().root_id() != expected_root
                || child.lineage().depth() != expected_depth
                || child.lineage().remaining_depth() != expected_remaining
            {
                return Err(DomainError::InvariantViolated {
                    reason: "planned child lineage does not match its parent plan",
                });
            }
        }
        self.require_step_claim_fence(coordinates.step_id(), plan.active_claim_fence())?;
        Ok(vec![CeremonyEvent::ChildSpawnPlanned(ChildSpawnPlanned {
            plan: plan.clone(),
            planned_at: command.now,
        })])
    }

    pub(super) fn decide_adopt_child_plan(
        &self,
        command: &AdoptChildSpawnPlan,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        let group = self
            .child_groups
            .get(&command.group_id)
            .ok_or(DomainError::NotFound {
                what: "child_spawn_group",
            })?;
        if group.adopted_claim_fence() == &command.claim_fence {
            return Ok(Vec::new());
        }
        let coordinates = group.plan().coordinates();
        let record = self
            .step_record(coordinates.step_id())
            .ok_or(DomainError::NotFound {
                what: "ceremony_child_spawn.step_record",
            })?;
        if coordinates.state_visit() != self.current_state_visit
            || coordinates.state_iteration() != self.current_state_iteration
            || coordinates.state_visit() != record.state_visit()
            || coordinates.state_iteration() != record.state_iteration()
            || coordinates.step_iteration() != record.iteration()
        {
            return Err(DomainError::InvariantViolated {
                reason: "cannot adopt a child spawn plan from another visit or iteration",
            });
        }
        self.require_step_claim_fence(coordinates.step_id(), &command.claim_fence)?;
        Ok(vec![CeremonyEvent::ChildSpawnPlanAdopted(
            ChildSpawnPlanAdopted {
                group_id: command.group_id.clone(),
                claim_fence: command.claim_fence.clone(),
                adopted_at: command.now,
            },
        )])
    }

    pub(super) fn decide_accept_child_completion(
        &self,
        command: &AcceptChildCompletion,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        let completion = &command.completion;
        let group = self
            .child_groups
            .get(completion.group_id())
            .ok_or(DomainError::NotFound {
                what: "child_spawn_group",
            })?;
        if !group
            .plan()
            .children()
            .iter()
            .any(|child| child.child_id() == completion.child_id())
        {
            return Err(DomainError::InvariantViolated {
                reason: "child completion does not belong to the sealed spawn plan",
            });
        }
        if let Some(existing) = group.completions().get(completion.child_id()) {
            return if existing == completion {
                Ok(Vec::new())
            } else {
                Err(DomainError::InvariantViolated {
                    reason: "child ceremony already has a different accepted terminal",
                })
            };
        }
        Ok(vec![CeremonyEvent::ChildCompletionAccepted(
            ChildCompletionAccepted {
                completion: completion.clone(),
                accepted_at: command.now,
            },
        )])
    }
}
