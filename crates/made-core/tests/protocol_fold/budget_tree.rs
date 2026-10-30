use made_core::entities::{ProtocolInstance, PublishedProtocolDefinition};
use made_core::value_objects::{
    BudgetAccountId, ChildDepth, ChildDepthBudget, ChildGroupId, ChildPosition, ProtocolContext,
    ProtocolId, ProtocolLineage,
};

use super::fixture::{at, definition};

#[test]
fn a_child_reopens_with_the_exact_root_budget_account() {
    let published = PublishedProtocolDefinition::seal(definition()).unwrap();
    let root_id = ProtocolId::new("budget-root").unwrap();
    let account_id = BudgetAccountId::for_root(&root_id).unwrap();
    let root_events = ProtocolInstance::decide_start_bound_budgeted(
        root_id.clone(),
        &published,
        ProtocolContext::empty(),
        account_id.clone(),
        None,
        at(0),
    )
    .unwrap();
    let root = ProtocolInstance::rehydrate(root_events.iter()).unwrap();

    let child_id = ProtocolId::new("budget-child").unwrap();
    let lineage = ProtocolLineage::new(
        root_id.clone(),
        root_id,
        ChildGroupId::new("a".repeat(64)).unwrap(),
        ChildPosition::new(0),
        ChildDepth::FIRST,
        ChildDepthBudget::new(1).unwrap(),
    )
    .unwrap();
    let child_events = ProtocolInstance::decide_start_bound_budgeted_child(
        child_id,
        &published,
        ProtocolContext::empty(),
        lineage,
        account_id.clone(),
        None,
        at(1),
    )
    .unwrap();
    let child = ProtocolInstance::rehydrate(child_events.iter()).unwrap();

    assert_eq!(root.budget_account_id(), Some(&account_id));
    assert_eq!(child.budget_account_id(), Some(&account_id));
    assert_eq!(child.lineage().unwrap().root_id(), root.id());
}
