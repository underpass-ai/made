//! Which authorization action a tool call is admitted under.
//!
//! A protected backend authorizes every business tool as one action of
//! the policy, and a grant names actions, not tools. The rule that joins
//! the two vocabularies is written once here, in names only, so that the
//! build without an engine (the gRPC-only client) can still say which
//! action a listed tool needs and the embedded authorizer, the terminal
//! `grant` command and discovery all read the same table.

use super::tool_names::is_server_tool;

/// The action a tool is authorized as, spelled as the policy and
/// `made_issue_authorization_grant` spell it.
///
/// `None` for the two server-owned tools, which reach no engine, and for
/// `made_approve_authorization_operation`, which approves another
/// operation under a separation rule rather than being an action of its
/// own. Every other tool is its own name without the `made_` prefix,
/// except the few whose tool and action were named differently.
#[must_use]
pub(crate) fn action_name_for_tool(tool: &str) -> Option<&str> {
    match tool {
        "made_get_budget_report" | "made_list_pending_budget_reservations" => Some("read_budget"),
        "made_get_authorization_policy" => Some("read_authorization_policy"),
        "made_issue_authorization_grant" => Some("issue_authorization_grant"),
        "made_revoke_authorization_grant" => Some("revoke_authorization_grant"),
        "made_list_authorization_decisions" => Some("read_authorization_decisions"),
        "made_approve_authorization_operation" => None,
        name if is_server_tool(name) => None,
        name => name.strip_prefix("made_"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::authorization_actions::GRANT_ACTIONS;
    use crate::protocol::tool_names::GRPC_TOOL_NAMES;

    /// A tool the catalog lists is a tool a grant can name: every one
    /// maps to an action the grant schema accepts, so a person told
    /// "grant `x`" can always do so.
    #[test]
    fn every_catalog_tool_maps_to_an_action_a_grant_accepts() {
        for tool in GRPC_TOOL_NAMES {
            if tool == "made_approve_authorization_operation" {
                assert_eq!(action_name_for_tool(tool), None);
                continue;
            }
            let action = action_name_for_tool(tool)
                .unwrap_or_else(|| panic!("{tool} is authorized as no action"));
            assert!(
                GRANT_ACTIONS.contains(&action),
                "{tool} is authorized as `{action}`, which the grant schema does not accept"
            );
        }
        assert_eq!(action_name_for_tool("made_discover_capabilities"), None);
        assert_eq!(action_name_for_tool("made_get_help"), None);
        assert_eq!(action_name_for_tool("not_a_made_tool"), None);
    }

    /// The embedded engine parses each name as its own action, so the
    /// table above and the domain enumeration cannot drift apart.
    #[cfg(feature = "embedded")]
    #[test]
    fn every_action_name_is_a_domain_action() {
        for tool in GRPC_TOOL_NAMES {
            let Some(action) = action_name_for_tool(tool) else {
                continue;
            };
            let parsed: Result<made_core::value_objects::AuthorizationAction, _> =
                serde_json::from_value(serde_json::Value::String(action.to_owned()));
            assert!(parsed.is_ok(), "{tool}: `{action}` is not a domain action");
        }
    }
}
