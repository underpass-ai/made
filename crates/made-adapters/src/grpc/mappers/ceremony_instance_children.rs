use made_core::value_objects::{
    CeremonyLineage, ChildCompletionRef, ChildGroupState, PlannedChild,
};
use made_proto::v1 as pb;

use super::attributes::attributes_to_struct;
use super::ceremony_instance::{moment, recollection_state_from};

pub(super) fn lineage_state_from(lineage: &CeremonyLineage) -> pb::CeremonyLineageState {
    pb::CeremonyLineageState {
        root_id: lineage.root_id().as_str().to_owned(),
        parent_id: lineage.parent_id().as_str().to_owned(),
        group_id: lineage.group_id().as_str().to_owned(),
        position: u32::from(lineage.position().get()),
        depth: u32::from(lineage.depth().get()),
        remaining_depth: u32::from(lineage.remaining_depth().get()),
    }
}

fn planned_child_state_from(child: &PlannedChild) -> pb::PlannedCeremonyChildState {
    pb::PlannedCeremonyChildState {
        child_id: child.child_id().as_str().to_owned(),
        position: u32::from(child.position().get()),
        ceremony: child.ceremony().as_str().to_owned(),
        version: child.version().as_str().to_owned(),
        definition_digest: child.digest().to_hex(),
        context: Some(attributes_to_struct(child.context().attributes())),
        lineage: Some(lineage_state_from(child.lineage())),
        recollection: child.recollection().map(recollection_state_from),
        opened_at: moment(child.opened_at()),
    }
}

pub fn child_completion_state_from(completion: &ChildCompletionRef) -> pb::ChildCompletionState {
    pb::ChildCompletionState {
        group_id: completion.group_id().as_str().to_owned(),
        child_id: completion.child_id().as_str().to_owned(),
        terminal_event_id: completion.terminal_event_id().as_str().to_owned(),
        terminal_record_hash: completion.terminal_record_hash().as_bytes().to_vec(),
    }
}

pub(super) fn child_group_state_from(group: &ChildGroupState) -> pb::CeremonyChildGroupState {
    let plan = group.plan();
    let coordinates = plan.coordinates();
    pb::CeremonyChildGroupState {
        group_id: plan.group_id().as_str().to_owned(),
        step_id: coordinates.step_id().as_str().to_owned(),
        state_visit: coordinates.state_visit().get(),
        state_iteration: coordinates.state_iteration().get(),
        step_iteration: coordinates.step_iteration().get(),
        active_claim_fence: plan.active_claim_fence().as_str().to_owned(),
        adopted_claim_fence: group.adopted_claim_fence().as_str().to_owned(),
        children: plan
            .children()
            .iter()
            .map(planned_child_state_from)
            .collect(),
        max_children: u32::from(plan.max_children().get()),
        max_depth: u32::from(plan.max_depth().get()),
        completions: group
            .completions()
            .values()
            .map(child_completion_state_from)
            .collect(),
    }
}
