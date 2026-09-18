use serde_json::{json, Value};

const ROUNDTABLE_FIXED_ORDER_FRAGMENT: &str = include_str!("fragments/roundtable_fixed_order.yaml");
const BROADCAST_COLLECT_FRAGMENT: &str = include_str!("fragments/broadcast_collect.yaml");
const GROUP_CHAT_FRAGMENT: &str = include_str!("fragments/group_chat.yaml");
const MAKER_CHECKER_FRAGMENT: &str = include_str!("fragments/maker_checker.yaml");
const HANDOFF_FRAGMENT: &str = include_str!("fragments/handoff.yaml");
const MAGENTIC_FRAGMENT: &str = include_str!("fragments/magentic.yaml");

pub(crate) const ROUNDTABLE_FIXED_ORDER_ID: &str = "roundtable_fixed_order";

pub(crate) fn design_pattern_catalog() -> Vec<Value> {
    let mut catalog = vec![json!({
        "id": ROUNDTABLE_FIXED_ORDER_ID,
        "scope": "ceremony",
        "description": "One speaking turn per participant in declaration order; every turn after the first receives the prior transcript.",
        "definition_fragment_yaml": ROUNDTABLE_FIXED_ORDER_FRAGMENT,
    })];
    catalog.extend([
        stage(
            "broadcast_collect",
            "Independent fan-out followed by one collecting synthesis step.",
            BROADCAST_COLLECT_FRAGMENT,
        ),
        stage(
            "group_chat",
            "Manager-selected speakers with bounded turns, early stop and fallback.",
            GROUP_CHAT_FRAGMENT,
        ),
        stage(
            "maker_checker",
            "Bounded maker/checker revisions with acceptance or escalation.",
            MAKER_CHECKER_FRAGMENT,
        ),
        stage(
            "handoff",
            "Output-routed role handoffs with a human exit and bounce budget.",
            HANDOFF_FRAGMENT,
        ),
        stage(
            "magentic",
            "Manager-owned task ledger with dynamic workers and stalled fallback.",
            MAGENTIC_FRAGMENT,
        ),
    ]);
    catalog
}

fn stage(id: &str, description: &str, source: &str) -> Value {
    json!({ "id": id, "scope": "stage", "description": description, "definition_fragment_yaml": source })
}
