use anyhow::{ensure, Context, Result};
use made_proto::v1::{
    AuthorizationScope, GetAuthorizationPolicyRequest, IssueAuthorizationGrantRequest,
};
use prost_types::Timestamp;
use tracing::info;

use super::E2eClient;

const POLICY_ID: &str = "made-e2e-policy";
const OWNER_ID: &str = "made-e2e-owner";
const BUSINESS_GRANT_ID: &str = "made-e2e-business-owner";

// These are the operations exercised by the Compose scenarios. Keeping the
// grant explicit makes additions fail closed until the fixture is reviewed.
const COMPOSE_ACTIONS: &[&str] = &[
    "apply_ceremony_transition",
    "create_council",
    "delete_council",
    "deliberate",
    "generate_ceremony_report",
    "get_ceremony_instance",
    "list_councils",
    "orchestrate",
    "publish_ceremony_definition",
    "read_ceremony_events",
    "register_agent",
    "register_contract",
    "run_ceremony",
    "run_ceremony_step",
    "run_council_decision",
    "start_published_ceremony",
    "stream_ceremony",
    "verify_ceremony_journal",
];

pub(crate) async fn provision_compose_business_grant(client: &mut E2eClient) -> Result<()> {
    let policy = client
        .get_authorization_policy(GetAuthorizationPolicyRequest {})
        .await
        .context("read bootstrapped authorization policy over public gRPC")?
        .into_inner()
        .policy
        .context("authorization policy response omitted policy")?;
    ensure!(policy.policy_id == POLICY_ID, "unexpected policy id");
    let owner = policy.owner.context("authorization policy omitted owner")?;
    ensure!(owner.principal_id == OWNER_ID, "unexpected policy owner");
    ensure!(
        owner.kind == "trusted_host",
        "policy owner is not a trusted host"
    );
    ensure!(
        owner.authentication_method == "mutual_tls",
        "policy owner is not authenticated with mTLS"
    );

    let issued = client
        .issue_authorization_grant(IssueAuthorizationGrantRequest {
            grant_id: BUSINESS_GRANT_ID.to_owned(),
            grantee_id: OWNER_ID.to_owned(),
            actions: COMPOSE_ACTIONS
                .iter()
                .map(|action| (*action).to_owned())
                .collect(),
            scope: Some(AuthorizationScope {
                kind: "global".to_owned(),
                ..AuthorizationScope::default()
            }),
            // Epoch is deliberate: the request remains byte-for-byte stable
            // when the same Compose store is exercised more than once.
            valid_from: Some(Timestamp {
                seconds: 0,
                nanos: 0,
            }),
            valid_until: None,
            delegation_depth: 0,
            parent_grant_id: None,
        })
        .await
        .context("issue explicit Compose business grant over public gRPC")?
        .into_inner();

    info!(
        policy_id = POLICY_ID,
        grant_id = BUSINESS_GRANT_ID,
        existing = issued.existing,
        version = issued.version,
        "Compose business grant is ready"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::COMPOSE_ACTIONS;

    #[test]
    fn compose_grant_is_sorted_unique_and_excludes_administration() {
        assert!(COMPOSE_ACTIONS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(!COMPOSE_ACTIONS.contains(&"issue_authorization_grant"));
        assert!(!COMPOSE_ACTIONS.contains(&"revoke_authorization_grant"));
    }
}
