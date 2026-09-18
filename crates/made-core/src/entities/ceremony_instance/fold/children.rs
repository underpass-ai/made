use crate::entities::ceremony_events::{
    ChildCompletionAccepted, ChildSpawnPlanAdopted, ChildSpawnPlanned,
};
use crate::entities::CeremonyInstance;
use crate::value_objects::ChildGroupState;

impl CeremonyInstance {
    pub(super) fn apply_child_spawn_planned(&mut self, event: &ChildSpawnPlanned) {
        self.child_groups
            .entry(event.plan.group_id().clone())
            .or_insert_with(|| ChildGroupState::new(event.plan.clone()));
        self.updated_at = event.planned_at;
    }

    pub(super) fn apply_child_spawn_plan_adopted(&mut self, event: &ChildSpawnPlanAdopted) {
        if let Some(group) = self.child_groups.get_mut(&event.group_id) {
            group.adopt(event.claim_fence.clone());
            self.updated_at = event.adopted_at;
        }
    }

    pub(super) fn apply_child_completion_accepted(&mut self, event: &ChildCompletionAccepted) {
        if let Some(group) = self.child_groups.get_mut(event.completion.group_id()) {
            group.accept(event.completion.clone());
            self.updated_at = event.accepted_at;
        }
    }
}
