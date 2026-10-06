//! The discovery entry that says what this session may do.
//!
//! The backend knows the policy: which principal it acts as and what a
//! live grant admits for it. The server knows the catalog: which tools
//! this process lists. The one fact a session wants is the join, the
//! listed tools it will be refused for, and only here are both halves
//! in hand.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::protocol::{action_name_for_tool, GRPC_TOOL_NAMES};

/// The backend's summary with `listed_tools_without_grant` added: every
/// tool `serves` admits whose action is not among `granted_actions`.
/// A backend that could not read its policy is reported as such rather
/// than as a host that holds nothing.
pub(super) fn authorization_entry(
    serves: impl Fn(&str) -> bool,
    summary: Result<Value, String>,
) -> Value {
    let mut summary = match summary {
        Ok(summary) => summary,
        Err(reason) => return json!({ "unavailable": reason }),
    };
    let granted: BTreeSet<&str> = summary["granted_actions"]
        .as_array()
        .map(|actions| actions.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let without_grant: Vec<Value> = GRPC_TOOL_NAMES
        .iter()
        .filter(|name| serves(name))
        .filter(|name| action_name_for_tool(name).is_some_and(|action| !granted.contains(action)))
        .map(|name| Value::String((*name).to_owned()))
        .collect();
    if let Some(object) = summary.as_object_mut() {
        object.insert(
            "listed_tools_without_grant".to_owned(),
            Value::Array(without_grant),
        );
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_listed_tools_without_a_grant_are_the_served_ones_whose_action_is_not_granted() {
        let served = |tool: &str| {
            matches!(
                tool,
                "made_design_ceremony" | "made_cancel_ceremony" | "made_get_budget_report"
            )
        };
        let entry = authorization_entry(
            served,
            Ok(json!({ "granted_actions": ["design_ceremony", "read_budget"] })),
        );
        assert_eq!(
            entry["listed_tools_without_grant"],
            json!(["made_cancel_ceremony"])
        );
        assert_eq!(
            entry["granted_actions"],
            json!(["design_ceremony", "read_budget"])
        );
    }

    #[test]
    fn an_unreadable_policy_is_said_not_answered_as_nothing_held() {
        let entry = authorization_entry(|_| true, Err("locked".to_owned()));
        assert_eq!(entry, json!({ "unavailable": "locked" }));
    }
}
