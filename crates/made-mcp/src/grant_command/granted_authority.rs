use std::collections::BTreeSet;

use made_core::entities::AuthorizationPolicy;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthorizationAction, AuthorizationPolicyVersion, AuthorizationScope,
};
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::protocol::{action_name_for_tool, GRPC_TOOL_NAMES};

/// What one principal may do under a policy, read from the policy
/// itself rather than remembered: the grants that name it, and the
/// actions the policy would admit for it at global scope right now.
///
/// Two readers want this. The terminal `grant` command shows it before
/// asking, so the person sees what is already held and what the grant
/// adds. Discovery carries it so a session learns which listed tools it
/// will be refused for before it is refused, instead of after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantedAuthority {
    principal: String,
    policy_id: Option<String>,
    policy_version: AuthorizationPolicyVersion,
    owner: bool,
    granted_actions: BTreeSet<AuthorizationAction>,
    grants: Vec<Value>,
}

impl GrantedAuthority {
    /// Ask the policy, action by action over everything the catalog can
    /// name, what it admits for `principal` at global scope now. The
    /// owner's administrative actions come out of the same question.
    #[must_use]
    pub fn project(
        policy: &AuthorizationPolicy,
        principal: &AuthenticatedPrincipal,
        now: OffsetDateTime,
    ) -> Self {
        let granted_actions = catalog_actions()
            .into_iter()
            .filter(|action| policy.permits(principal, *action, &AuthorizationScope::Global, now))
            .collect();
        let revoked: BTreeSet<&str> = policy
            .revoked_grant_ids()
            .map(made_core::value_objects::AuthorizationGrantId::as_str)
            .collect();
        let grants = policy
            .grants()
            .filter(|grant| grant.grantee() == principal.id())
            .map(|grant| {
                json!({
                    "grant_id": grant.id().as_str(),
                    "scope": grant.scope(),
                    "actions": grant.actions(),
                    "valid_until": grant.valid_until().and_then(|until| until.format(&Rfc3339).ok()),
                    "active": grant.is_active_at(now),
                    "revoked": revoked.contains(grant.id().as_str()),
                })
            })
            .collect();
        Self {
            principal: principal.id().as_str().to_owned(),
            policy_id: policy.id().map(|id| id.as_str().to_owned()),
            policy_version: policy.version(),
            owner: policy.owner() == Some(principal),
            granted_actions,
            grants,
        }
    }

    #[must_use]
    pub fn granted_actions(&self) -> &BTreeSet<AuthorizationAction> {
        &self.granted_actions
    }

    #[must_use]
    pub const fn is_owner(&self) -> bool {
        self.owner
    }

    /// The discovery entry, before the server adds which of the tools
    /// it lists have no grant behind them.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "principal": self.principal,
            "policy_id": self.policy_id,
            "policy_version": self.policy_version,
            "owner": self.owner,
            "granted_actions": self.granted_actions,
            "grants": self.grants,
        })
    }

    /// The same facts for a person at a terminal.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "Policy `{}` at version {}; principal `{}`{}.",
            self.policy_id.as_deref().unwrap_or("?"),
            self.policy_version.value(),
            self.principal,
            if self.owner {
                " owns it and may administer grants"
            } else {
                ""
            }
        )];
        if self.grants.is_empty() {
            lines.push("No grant names this principal.".to_owned());
        }
        for grant in &self.grants {
            let state = match (grant["revoked"].as_bool(), grant["active"].as_bool()) {
                (Some(true), _) => "revoked",
                (_, Some(false)) => "not active now",
                _ => "live",
            };
            lines.push(format!(
                "Grant `{}` ({state}): {} action(s) at scope {}{}.",
                grant["grant_id"].as_str().unwrap_or("?"),
                grant["actions"].as_array().map_or(0, Vec::len),
                scope_label(&grant["scope"]),
                grant["valid_until"]
                    .as_str()
                    .map(|until| format!(", until {until}"))
                    .unwrap_or_default()
            ));
        }
        let business: Vec<String> = self
            .granted_actions
            .iter()
            .filter(|action| !is_administrative(**action))
            .map(|action| action_name(*action))
            .collect();
        lines.push(if business.is_empty() {
            "Admitted at global scope now: no business action. Every business tool is refused \
             until a grant names its action."
                .to_owned()
        } else {
            format!(
                "Admitted at global scope now: {} business action(s): {}.",
                business.len(),
                business.join(", ")
            )
        });
        lines
    }
}

/// Every action a catalog tool is authorized as, in catalog order.
pub(super) fn catalog_actions() -> Vec<AuthorizationAction> {
    let mut seen = BTreeSet::new();
    GRPC_TOOL_NAMES
        .iter()
        .filter_map(|tool| action_name_for_tool(tool))
        .filter_map(|name| serde_json::from_value(Value::String(name.to_owned())).ok())
        .filter(|action| seen.insert(*action))
        .collect()
}

pub(super) fn is_administrative(action: AuthorizationAction) -> bool {
    matches!(
        action,
        AuthorizationAction::ReadAuthorizationPolicy
            | AuthorizationAction::IssueAuthorizationGrant
            | AuthorizationAction::RevokeAuthorizationGrant
            | AuthorizationAction::ReadAuthorizationDecisions
    )
}

/// The policy's spelling of an action.
pub(super) fn action_name(action: AuthorizationAction) -> String {
    serde_json::to_value(action)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// A scope as a person reads it.
pub(super) fn scope_label(scope: &Value) -> String {
    match scope["kind"].as_str() {
        Some("global") => "global".to_owned(),
        Some("ceremony") => format!(
            "ceremony `{}`",
            scope["ceremony_id"].as_str().unwrap_or("?")
        ),
        Some("ceremony_tree") => {
            format!(
                "ceremony tree `{}`",
                scope["root_id"].as_str().unwrap_or("?")
            )
        }
        Some("definition") => match scope["version"].as_str() {
            Some(version) => format!(
                "definition `{}` version `{version}`",
                scope["name"].as_str().unwrap_or("?")
            ),
            None => format!(
                "definition `{}`, any version",
                scope["name"].as_str().unwrap_or("?")
            ),
        },
        Some(kind) => format!("{kind} {scope}"),
        None => scope.to_string(),
    }
}
