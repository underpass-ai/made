//! Where a host's authority comes from.
//!
//! Bootstrapping a store opens its authorization policy and makes the
//! trusted host its owner; it grants nothing. Every business tool is
//! then refused until a grant names its action. Over MCP a grant is
//! issued with `made_issue_authorization_grant`, which the plugin's
//! `core` profile hides on purpose: a session that can widen its own
//! authority has no authority boundary. The other channel is a person
//! at a terminal the agent's session cannot write to, running
//! `made-mcp grant`. These two strings are how refusals, discovery and
//! help name that channel, spelled once so they cannot disagree.

/// The command a person runs, as discovery, refusals and usage spell it.
pub const GRANT_COMMAND: &str = "made-mcp grant <store> --profile core | --actions <a,b,...> \
     [--grantee <principal>] [--scope global|ceremony:<id>|definition:<name>[@<version>]] \
     [--valid-until <rfc3339>] [--grant-id <id>] [--show]";

/// The plugin wrapper that fills in the store and the policy.
pub const GRANT_SCRIPT: &str = "scripts/made-grant.sh";

/// The one line a refusal and the agent help both end on: what the
/// session is to do about an action it holds no grant for.
#[must_use]
pub fn grant_remedy(action: &str) -> String {
    format!(
        "A person issues a grant from their own terminal: `made-mcp grant <store> --profile core` \
         for the ordinary route, or `--actions {action}` for this call (the plugin ships \
         {GRANT_SCRIPT}); then call again. This session cannot grant itself, and \
         `made_discover_capabilities` lists what it holds under `authorization`."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_remedy_names_the_command_the_script_and_the_action() {
        let remedy = grant_remedy("design_ceremony");
        assert!(
            remedy.contains("made-mcp grant <store> --profile core"),
            "{remedy}"
        );
        assert!(remedy.contains("--actions design_ceremony"), "{remedy}");
        assert!(remedy.contains(GRANT_SCRIPT), "{remedy}");
        assert!(GRANT_COMMAND.starts_with("made-mcp grant <store>"));
    }
}
