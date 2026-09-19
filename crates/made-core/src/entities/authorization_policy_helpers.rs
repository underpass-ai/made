use std::collections::BTreeMap;

use time::OffsetDateTime;

use crate::value_objects::{AuthorizationAction, SeparationRule};
use crate::DomainError;

pub(super) fn validity_contains(
    parent: Option<OffsetDateTime>,
    child: Option<OffsetDateTime>,
) -> bool {
    match (parent, child) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(parent), Some(child)) => child <= parent,
    }
}

pub(super) fn owner_permits(action: AuthorizationAction) -> bool {
    matches!(
        action,
        AuthorizationAction::ReadAuthorizationPolicy
            | AuthorizationAction::IssueAuthorizationGrant
            | AuthorizationAction::RevokeAuthorizationGrant
            | AuthorizationAction::ReadAuthorizationDecisions
    )
}

pub(super) fn separation_rule_map(
    rules: Vec<SeparationRule>,
) -> Result<BTreeMap<AuthorizationAction, SeparationRule>, DomainError> {
    let count = rules.len();
    let mapped = rules
        .into_iter()
        .map(|rule| (rule.execution_action(), rule))
        .collect::<BTreeMap<_, _>>();
    if mapped.len() != count {
        return Err(DomainError::Conflict {
            what: "authorization_separation_rule",
        });
    }
    Ok(mapped)
}
