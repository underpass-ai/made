use crate::entities::CeremonyInstance;
use crate::value_objects::{ChildGroupId, ChildSpawnCoordinates, ChildrenCompletedCondition};

impl CeremonyInstance {
    #[must_use]
    pub fn children_completed_guard_is_satisfied(
        &self,
        condition: &ChildrenCompletedCondition,
    ) -> bool {
        let Some(record) = self.step_record(condition.step_id()) else {
            return false;
        };
        let coordinates = ChildSpawnCoordinates::new(
            condition.step_id().clone(),
            self.current_state_visit,
            self.current_state_iteration,
            record.iteration(),
        );
        let group_id = ChildGroupId::derive(&self.id, &coordinates);
        self.child_groups.get(&group_id).is_some_and(|group| {
            group.plan().coordinates() == &coordinates
                && condition
                    .join()
                    .is_satisfied(group.completions().len(), group.plan().children().len())
        })
    }
}
