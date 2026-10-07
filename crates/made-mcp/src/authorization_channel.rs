//! Where a host's authority comes from.
//!
//! Bootstrapping a store opens its authorization policy and makes the
//! trusted host its owner; it grants nothing. Every business tool is
//! then refused until a grant names its action. Over MCP a grant is
//! issued with `made_issue_authorization_grant`, which the plugin's
//! `core` profile hides on purpose: a session that can widen its own
//! authority has no authority boundary. The other channel is a person
//! at a terminal the agent's session cannot write to, running
//! `made-mcp grant`. These two strings are how discovery and help name
//! that channel, spelled once so they cannot disagree. The refusal
//! itself is worded in `made_adapters::authorization_denial`, shared
//! with the gRPC service so both arms answer the same words.

/// The command a person runs, as discovery, refusals and usage spell it.
pub const GRANT_COMMAND: &str = "made-mcp grant <store> --profile core | --actions <a,b,...> \
     [--grantee <principal>] [--scope global|ceremony:<id>|definition:<name>[@<version>]] \
     [--valid-until <rfc3339>] [--grant-id <id>] [--show]";

/// The plugin wrapper that fills in the store and the policy.
pub const GRANT_SCRIPT: &str = "scripts/made-grant.sh";
