//! Which part of the catalog a server shows the host that started it.
//!
//! A host loads every tool it is listed into its model's context before
//! the first question is asked. The full catalog is over a hundred tools
//! and, with their schemas, tens of thousands of tokens — a quarter of a
//! working context spent before any work. A profile is the server's way
//! of listing only what the session is for, without a second catalog
//! that could drift from the first: the profile admits names, and every
//! name it admits is still answered by the same backend, validated by the
//! same schema and described by the same discovery.
//!
//! Hiding a tool is not a permission. Authorization decides what the
//! principal may do; the profile decides what the host is told about.
//! A hidden tool called by name is refused with the profile named, so a
//! host that read an older guide learns what to change rather than
//! meeting a tool that silently does not exist.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::guidance::capability_group::CAPABILITY_GROUPS;
use crate::protocol::{
    is_server_tool, APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL,
    CANCEL_CEREMONY_TOOL, CLAIM_CEREMONY_STEP_TOOL, COMPLETE_CEREMONY_STEP_TOOL,
    DEFER_CEREMONY_GUARD_TOOL, DESIGN_CEREMONY_TOOL, DISCOVER_CAPABILITIES_TOOL,
    EXPLAIN_CEREMONY_DRAFT_TOOL, GENERATE_CEREMONY_REPORT_TOOL, GET_CEREMONY_DEFINITION_TOOL,
    GET_CEREMONY_INSTANCE_TOOL, GET_CEREMONY_TRANSCRIPT_TOOL, GET_HELP_TOOL,
    LIST_CEREMONY_DEFINITIONS_TOOL, LIST_CEREMONY_INSTANCES_TOOL, PAUSE_CEREMONY_TOOL,
    PUBLISH_CEREMONY_DEFINITION_TOOL, READ_CEREMONY_EVENTS_TOOL, RENEW_CEREMONY_STEP_LEASE_TOOL,
    RESUME_CEREMONY_TOOL, START_PUBLISHED_CEREMONY_TOOL, VALIDATE_CEREMONY_DRAFT_TOOL,
    VERIFY_CEREMONY_JOURNAL_TOOL,
};

/// Selects the profile: `full` (the default), `core`, or `core` widened
/// with capability groups, written `core+<group>[+<group>...]`.
pub const TOOL_PROFILE_ENV: &str = "MADE_MCP_TOOL_PROFILE";

/// The name of the profile that shows everything the backend serves.
pub const FULL_PROFILE_NAME: &str = "full";

/// The name of the profile that shows the ordinary route and nothing else.
pub const CORE_PROFILE_NAME: &str = "core";

/// The tools of the ordinary route: find out what this server is, design
/// and publish a definition, run a session step by step, let a person
/// decide, and read the record back. Twenty-four names, every one on
/// every surface, and no council, system, artifact, budget, intervention
/// or integrator-loop tool among them — those are groups a session asks
/// for by name.
pub(crate) const CORE_TOOL_NAMES: &[&str] = &[
    DISCOVER_CAPABILITIES_TOOL,
    GET_HELP_TOOL,
    DESIGN_CEREMONY_TOOL,
    VALIDATE_CEREMONY_DRAFT_TOOL,
    EXPLAIN_CEREMONY_DRAFT_TOOL,
    PUBLISH_CEREMONY_DEFINITION_TOOL,
    LIST_CEREMONY_DEFINITIONS_TOOL,
    GET_CEREMONY_DEFINITION_TOOL,
    START_PUBLISHED_CEREMONY_TOOL,
    GET_CEREMONY_INSTANCE_TOOL,
    LIST_CEREMONY_INSTANCES_TOOL,
    CLAIM_CEREMONY_STEP_TOOL,
    RENEW_CEREMONY_STEP_LEASE_TOOL,
    COMPLETE_CEREMONY_STEP_TOOL,
    APPLY_CEREMONY_TRANSITION_TOOL,
    PAUSE_CEREMONY_TOOL,
    RESUME_CEREMONY_TOOL,
    CANCEL_CEREMONY_TOOL,
    APPROVE_CEREMONY_GUARD_TOOL,
    DEFER_CEREMONY_GUARD_TOOL,
    READ_CEREMONY_EVENTS_TOOL,
    VERIFY_CEREMONY_JOURNAL_TOOL,
    GET_CEREMONY_TRANSCRIPT_TOOL,
    GENERATE_CEREMONY_REPORT_TOOL,
];

/// The names a server lists, chosen once at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolProfile {
    name: String,
    /// `None` admits every tool the backend serves.
    admitted: Option<BTreeSet<&'static str>>,
}

impl Default for ToolProfile {
    fn default() -> Self {
        Self::full()
    }
}

impl ToolProfile {
    /// Everything the backend serves.
    #[must_use]
    pub fn full() -> Self {
        Self {
            name: FULL_PROFILE_NAME.to_owned(),
            admitted: None,
        }
    }

    /// The ordinary route only.
    #[must_use]
    pub fn core() -> Self {
        Self {
            name: CORE_PROFILE_NAME.to_owned(),
            admitted: Some(CORE_TOOL_NAMES.iter().copied().collect()),
        }
    }

    /// Read the profile from [`TOOL_PROFILE_ENV`]; unset or blank is
    /// `full`, and a spec that names a group this build does not have
    /// is an error rather than a smaller catalog nobody asked for.
    pub fn from_env() -> Result<Self, String> {
        match std::env::var(TOOL_PROFILE_ENV) {
            Ok(spec) if !spec.trim().is_empty() => Self::parse(&spec),
            _ => Ok(Self::full()),
        }
    }

    /// Parse `full`, `core` or `core+<group>[+<group>...]`.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        if spec.eq_ignore_ascii_case(FULL_PROFILE_NAME) {
            return Ok(Self::full());
        }
        let mut parts = spec.split('+').map(str::trim);
        let base = parts.next().unwrap_or_default();
        if !base.eq_ignore_ascii_case(CORE_PROFILE_NAME) {
            return Err(format!(
                "{TOOL_PROFILE_ENV} must be `{FULL_PROFILE_NAME}`, `{CORE_PROFILE_NAME}` or \
                 `{CORE_PROFILE_NAME}+<group>[+<group>...]`; got `{spec}`"
            ));
        }
        let mut profile = Self::core();
        let mut widened = Vec::new();
        for group_id in parts {
            if group_id.is_empty() {
                return Err(format!(
                    "{TOOL_PROFILE_ENV} has an empty group name in `{spec}`"
                ));
            }
            let Some(group) = CAPABILITY_GROUPS.iter().find(|group| group.id == group_id) else {
                return Err(format!(
                    "{TOOL_PROFILE_ENV} names unknown capability group `{group_id}`; known groups: {}",
                    CAPABILITY_GROUPS
                        .iter()
                        .map(|group| group.id)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            };
            if let Some(admitted) = profile.admitted.as_mut() {
                admitted.extend(group.tools.iter().copied());
            }
            widened.push(group.id);
        }
        if !widened.is_empty() {
            profile.name = format!("{CORE_PROFILE_NAME}+{}", widened.join("+"));
        }
        Ok(profile)
    }

    /// The profile's name as a host would write it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether this profile lists `tool`. Server-owned tools are always
    /// listed: a host must be able to ask what it is talking to.
    #[must_use]
    pub fn admits(&self, tool: &str) -> bool {
        if is_server_tool(tool) {
            return true;
        }
        self.admitted
            .as_ref()
            .is_none_or(|admitted| admitted.contains(tool))
    }

    /// The server-owned discovery entry: what this profile is, what it
    /// keeps from the host among what the backend serves, and how to
    /// widen it. `served` answers whether the backend serves a tool.
    #[must_use]
    pub fn describe(&self, served: impl Fn(&str) -> bool) -> Value {
        let hidden = |group_tools: &[&'static str]| {
            group_tools
                .iter()
                .copied()
                .filter(|tool| served(tool) && !self.admits(tool))
                .collect::<Vec<_>>()
        };
        let hidden_groups = CAPABILITY_GROUPS
            .iter()
            .filter_map(|group| {
                let hidden = hidden(group.tools);
                (!hidden.is_empty()).then(|| {
                    json!({
                        "id": group.id,
                        "hidden_tools": hidden,
                    })
                })
            })
            .collect::<Vec<_>>();
        let hidden_tool_count = hidden_groups
            .iter()
            .map(|group| group["hidden_tools"].as_array().map_or(0, Vec::len))
            .sum::<usize>();
        json!({
            "name": self.name,
            "env": TOOL_PROFILE_ENV,
            "hidden_tool_count": hidden_tool_count,
            "hidden_groups": hidden_groups,
            "widen": format!(
                "Restart the server with {TOOL_PROFILE_ENV}={FULL_PROFILE_NAME} for every tool, \
                 or {TOOL_PROFILE_ENV}={CORE_PROFILE_NAME}+<group>[+<group>] to add a capability \
                 group by its id. A hidden tool called by name is refused, never served under \
                 another name."
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{available_tool_catalog, is_grpc_tool, AWAIT_INTEGRATOR_ATTENTION_TOOL};

    #[test]
    fn full_admits_everything_the_backend_serves() {
        let profile = ToolProfile::full();
        for tool in available_tool_catalog(is_grpc_tool) {
            assert!(profile.admits(tool["name"].as_str().unwrap()));
        }
        assert_eq!(profile.name(), "full");
    }

    #[test]
    fn core_names_exist_in_the_catalog_and_hide_the_rest() {
        let profile = ToolProfile::core();
        let catalog: BTreeSet<String> = available_tool_catalog(is_grpc_tool)
            .into_iter()
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect();
        for name in CORE_TOOL_NAMES {
            assert!(catalog.contains(*name), "{name} is not in the catalog");
            assert!(profile.admits(name));
        }
        assert!(!profile.admits(AWAIT_INTEGRATOR_ATTENTION_TOOL));
        assert!(!profile.admits("made_deliberate"));
        let listed = catalog.iter().filter(|name| profile.admits(name)).count();
        assert_eq!(listed, CORE_TOOL_NAMES.len());
    }

    #[test]
    fn server_tools_are_admitted_by_every_profile() {
        let profile = ToolProfile::parse("core").unwrap();
        assert!(profile.admits(DISCOVER_CAPABILITIES_TOOL));
        assert!(profile.admits(GET_HELP_TOOL));
    }

    #[test]
    fn core_widens_by_capability_group() {
        let profile = ToolProfile::parse("core+integrator_loop+ceremony_participation").unwrap();
        assert_eq!(
            profile.name(),
            "core+integrator_loop+ceremony_participation"
        );
        assert!(profile.admits(AWAIT_INTEGRATOR_ATTENTION_TOOL));
        assert!(profile.admits("made_request_ceremony_intervention"));
        assert!(!profile.admits("made_deliberate"));
    }

    #[test]
    fn unknown_group_and_unknown_base_are_refused() {
        let error = ToolProfile::parse("core+no_such_group").unwrap_err();
        assert!(error.contains("no_such_group"), "{error}");
        assert!(error.contains("integrator_loop"), "{error}");
        let error = ToolProfile::parse("minimal").unwrap_err();
        assert!(error.contains("`core`"), "{error}");
        let error = ToolProfile::parse("core+").unwrap_err();
        assert!(error.contains("empty group"), "{error}");
    }

    #[test]
    fn describe_counts_what_the_backend_serves_but_the_profile_hides() {
        let full = ToolProfile::full().describe(is_grpc_tool);
        assert_eq!(full["hidden_tool_count"], 0);
        assert_eq!(full["hidden_groups"].as_array().unwrap().len(), 0);

        let core = ToolProfile::core().describe(is_grpc_tool);
        let served = available_tool_catalog(is_grpc_tool).len();
        assert_eq!(
            core["hidden_tool_count"].as_u64().unwrap() as usize,
            served - CORE_TOOL_NAMES.len()
        );
        assert_eq!(core["env"], TOOL_PROFILE_ENV);
        let groups = core["hidden_groups"].as_array().unwrap();
        assert!(groups.iter().any(|group| group["id"] == "integrator_loop"));
        assert!(!groups.iter().any(|group| group["id"] == "self_description"));
        assert!(!groups
            .iter()
            .any(|group| group["id"] == "human_authorization"));

        // A backend that serves nothing beyond the core hides nothing.
        let narrow = ToolProfile::core().describe(|tool| CORE_TOOL_NAMES.contains(&tool));
        assert_eq!(narrow["hidden_tool_count"], 0);
    }
}
