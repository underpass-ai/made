use std::collections::BTreeSet;

use made_core::value_objects::AuthorizationAction;
use serde_json::Value;

use crate::embedded::embedded_supports_tool;
use crate::protocol::{action_name_for_tool, GRPC_TOOL_NAMES};
use crate::tool_profile::ToolProfile;

/// Which actions a grant names: the ones behind a tool profile, or a
/// list the person wrote out.
///
/// A profile is the natural unit: a person who starts the plugin with
/// the `core` profile is told twenty-two tools exist and should be able
/// to allow exactly those with one word. The spelling is the one
/// `MADE_MCP_TOOL_PROFILE` takes (`full`, `core`, `core+<group>`), so
/// what the host is shown and what it is allowed can be said the same
/// way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantSelection {
    /// The actions of every tool the profile lists on the embedded
    /// backend, the two server-owned tools aside (they reach no engine).
    Profile(String),
    /// Actions named one by one, as the policy spells them.
    Actions(Vec<String>),
}

impl GrantSelection {
    /// Parse `--actions a,b,c`: a non-empty, comma-separated list.
    pub fn parse_actions(list: &str) -> Result<Self, String> {
        let actions: Vec<String> = list
            .split(',')
            .map(str::trim)
            .filter(|action| !action.is_empty())
            .map(str::to_owned)
            .collect();
        if actions.is_empty() {
            return Err("--actions needs at least one action name".to_owned());
        }
        Ok(Self::Actions(actions))
    }

    /// The actions the selection names, each one a real action of the
    /// policy. An unknown name is an error rather than a smaller grant
    /// nobody asked for.
    pub fn actions(&self) -> Result<BTreeSet<AuthorizationAction>, String> {
        match self {
            Self::Profile(spec) => {
                // The catalog's names, not the server-owned pair (they
                // reach no engine and are in no group), kept to what
                // the embedded engine serves and the profile lists.
                let profile = ToolProfile::parse(spec)?;
                GRPC_TOOL_NAMES
                    .iter()
                    .filter(|name| embedded_supports_tool(name) && profile.admits(name))
                    .filter_map(|name| action_name_for_tool(name))
                    .map(parse_action)
                    .collect()
            }
            Self::Actions(names) => names.iter().map(|name| parse_action(name)).collect(),
        }
    }

    /// How the selection reads in a receipt.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Profile(spec) => format!("tool profile `{spec}`"),
            Self::Actions(names) => format!("{} named action(s)", names.len()),
        }
    }

    /// A short, legible stem for a default grant id.
    #[must_use]
    pub fn slug(&self) -> String {
        match self {
            Self::Profile(spec) => spec
                .chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() {
                        character.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect(),
            Self::Actions(names) if names.len() == 1 => names[0].replace('_', "-"),
            Self::Actions(names) => format!("{}-actions", names.len()),
        }
    }
}

fn parse_action(name: &str) -> Result<AuthorizationAction, String> {
    serde_json::from_value(Value::String(name.to_owned())).map_err(|_| {
        format!(
            "unknown action `{name}`; an action is a tool's name without the `made_` prefix \
             (for example `design_ceremony`), and `made_discover_capabilities` lists the tools"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_profile::CORE_TOOL_NAMES;

    #[test]
    fn the_core_profile_names_the_actions_of_its_twenty_two_business_tools() {
        let actions = GrantSelection::Profile("core".to_owned())
            .actions()
            .unwrap();
        // Twenty-four core tools, two of them server-owned.
        assert_eq!(actions.len(), CORE_TOOL_NAMES.len() - 2);
        assert!(actions.contains(&AuthorizationAction::DesignCeremony));
        assert!(actions.contains(&AuthorizationAction::ApproveCeremonyGuard));
        assert!(actions.contains(&AuthorizationAction::VerifyCeremonyJournal));
        assert!(!actions.contains(&AuthorizationAction::IssueAuthorizationGrant));
        assert!(!actions.contains(&AuthorizationAction::AwaitIntegratorAttention));
    }

    #[test]
    fn a_widened_profile_adds_its_groups_and_full_adds_the_administration() {
        let widened = GrantSelection::Profile("core+integrator_loop".to_owned())
            .actions()
            .unwrap();
        assert!(widened.contains(&AuthorizationAction::AwaitIntegratorAttention));
        assert!(!widened.contains(&AuthorizationAction::IssueAuthorizationGrant));
        let full = GrantSelection::Profile("full".to_owned())
            .actions()
            .unwrap();
        assert!(full.contains(&AuthorizationAction::IssueAuthorizationGrant));
        assert!(full.len() > widened.len());
    }

    #[test]
    fn named_actions_are_parsed_and_an_unknown_one_is_refused() {
        let actions =
            GrantSelection::parse_actions(" design_ceremony, publish_ceremony_definition ")
                .unwrap()
                .actions()
                .unwrap();
        assert_eq!(
            actions,
            BTreeSet::from([
                AuthorizationAction::DesignCeremony,
                AuthorizationAction::PublishCeremonyDefinition,
            ])
        );
        let error = GrantSelection::parse_actions("made_design_ceremony")
            .unwrap()
            .actions()
            .unwrap_err();
        assert!(
            error.contains("unknown action `made_design_ceremony`"),
            "{error}"
        );
        assert!(GrantSelection::parse_actions(" , ").is_err());
        let error = GrantSelection::Profile("minimal".to_owned())
            .actions()
            .unwrap_err();
        assert!(error.contains("`core`"), "{error}");
    }

    #[test]
    fn the_slug_is_legible_in_a_grant_id() {
        assert_eq!(
            GrantSelection::Profile("core+integrator_loop".to_owned()).slug(),
            "core-integrator-loop"
        );
        assert_eq!(
            GrantSelection::Actions(vec!["design_ceremony".to_owned()]).slug(),
            "design-ceremony"
        );
        assert_eq!(
            GrantSelection::Actions(vec!["a".to_owned(), "b".to_owned()]).slug(),
            "2-actions"
        );
    }
}
