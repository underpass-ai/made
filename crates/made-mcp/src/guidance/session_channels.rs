//! What an agent must know about the session it is on: which tools the
//! profile lists, and on which channel a person's decision is accepted.
//!
//! Both are facts about this process rather than about the engine, and
//! both are things an agent would otherwise work around — calling a
//! hidden tool under another name, or relaying an approval the server
//! will not take from it.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::authorization_channel::{GRANT_COMMAND, GRANT_SCRIPT};
use crate::human_approval_source::{TERMINAL_APPROVAL_COMMAND, TERMINAL_APPROVAL_SCRIPT};
use crate::protocol::{APPROVE_CEREMONY_GUARD_TOOL, DISCOVER_CAPABILITIES_TOOL};

pub(super) fn session_channel_guidance(
    names: &BTreeSet<String>,
    preconditions: &mut Vec<String>,
    authority_boundaries: &mut Vec<Value>,
) {
    preconditions.push(format!(
        "Read `tool_profile` in {DISCOVER_CAPABILITIES_TOOL}: a tool the backend serves but the profile hides is refused by name, never served under another. Tell the person which capability group to add (restart with MADE_MCP_TOOL_PROFILE=core+<group>, or full) rather than working around it."
    ));
    preconditions.push(format!(
        "Read `authorization` in {DISCOVER_CAPABILITIES_TOOL} when it is present: `listed_tools_without_grant` names the tools this session will be refused for lack of a grant, and a refusal names the action. Ask the person to run `{GRANT_COMMAND}` in their own terminal (the plugin ships {GRANT_SCRIPT}; `--profile core` is the ordinary route). This session cannot grant itself; widening the tool profile to issue a grant to yourself is not the remedy."
    ));
    authority_boundaries.push(json!({
        "rule": "Bootstrapping a store grants nothing. A listed tool is a tool the host may ask for, not one it may use; authority is a grant a person issued.",
        "forbidden_inference": "A tool that appears in tools/list is a tool this session is permitted to call."
    }));
    if names.contains(APPROVE_CEREMONY_GUARD_TOOL) {
        preconditions.push(format!(
            "Read `human_approval` in {DISCOVER_CAPABILITIES_TOOL} before a human guard is reached. With source `terminal`, {APPROVE_CEREMONY_GUARD_TOOL} is refused on this session: tell the person to run `{TERMINAL_APPROVAL_COMMAND}` (the plugin ships {TERMINAL_APPROVAL_SCRIPT}) in their own terminal, then read the instance again. With source `host`, record a person's decision only after they stated it in the current conversation, as role_kind human."
        ));
        authority_boundaries.push(json!({
            "rule": "A human approval recorded on the MCP session is the agent's statement about a person; a terminal approval is the person's own channel. Neither authenticates who sat there.",
            "forbidden_inference": "role_kind human on a relayed approval proves a person decided."
        }));
    }
}
