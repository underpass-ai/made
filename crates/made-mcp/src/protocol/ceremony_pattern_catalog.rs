use serde_json::{json, Value};

const ROUNDTABLE_FIXED_ORDER_FRAGMENT: &str = include_str!("fragments/roundtable_fixed_order.yaml");

pub(crate) const ROUNDTABLE_FIXED_ORDER_ID: &str = "roundtable_fixed_order";

pub(crate) fn design_pattern_catalog() -> Vec<Value> {
    vec![json!({
        "id": ROUNDTABLE_FIXED_ORDER_ID,
        "description": "One speaking turn per participant in declaration order; every turn after the first receives the prior transcript.",
        "definition_fragment_yaml": ROUNDTABLE_FIXED_ORDER_FRAGMENT,
    })]
}
