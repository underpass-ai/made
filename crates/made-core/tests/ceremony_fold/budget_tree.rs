use made_core::entities::{CeremonyInstance, PublishedCeremonyDefinition};
use made_core::value_objects::{
    BudgetAccountId, CeremonyContext, CeremonyId, CeremonyLineage, ChildDepth, ChildDepthBudget,
    ChildGroupId, ChildPosition,
};

use super::fixture::{at, definition};

#[test]
fn a_child_reopens_with_the_exact_root_budget_account() {
    let published = PublishedCeremonyDefinition::seal(definition()).unwrap();
    let root_id = CeremonyId::new("budget-root").unwrap();
    let account_id = BudgetAccountId::for_root(&root_id).unwrap();
    let root_events = CeremonyInstance::decide_start_bound_budgeted(
        root_id.clone(),
        &published,
        CeremonyContext::empty(),
        account_id.clone(),
        None,
        at(0),
    )
    .unwrap();
    let root = CeremonyInstance::rehydrate(root_events.iter()).unwrap();

    let child_id = CeremonyId::new("budget-child").unwrap();
    let lineage = CeremonyLineage::new(
        root_id.clone(),
        root_id,
        ChildGroupId::new("a".repeat(64)).unwrap(),
        ChildPosition::new(0),
        ChildDepth::FIRST,
        ChildDepthBudget::new(1).unwrap(),
    )
    .unwrap();
    let child_events = CeremonyInstance::decide_start_bound_budgeted_child(
        child_id,
        &published,
        CeremonyContext::empty(),
        lineage,
        account_id.clone(),
        None,
        at(1),
    )
    .unwrap();
    let child = CeremonyInstance::rehydrate(child_events.iter()).unwrap();

    assert_eq!(root.budget_account_id(), Some(&account_id));
    assert_eq!(child.budget_account_id(), Some(&account_id));
    assert_eq!(child.lineage().unwrap().root_id(), root.id());
}
