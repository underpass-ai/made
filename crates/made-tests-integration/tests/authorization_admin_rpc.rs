use made_proto::made_service_client::MadeServiceClient;
use made_proto::{
    AuthorizationScope, GetAuthorizationPolicyRequest, IssueAuthorizationGrantRequest,
    ListAuthorizationDecisionsRequest, RevokeAuthorizationGrantRequest,
};
use made_tests_integration::grpc_fixture::GrpcFixture;
use prost_types::Timestamp;

#[tokio::test]
async fn owner_administers_grants_and_reads_durable_decisions() {
    let fixture = GrpcFixture::start().await;
    let mut client = MadeServiceClient::new(fixture.channel);

    let issued = client
        .issue_authorization_grant(IssueAuthorizationGrantRequest {
            grant_id: "reader-a".to_owned(),
            grantee_id: "reader-a".to_owned(),
            actions: vec!["get_ceremony_instance".to_owned()],
            scope: Some(AuthorizationScope {
                kind: "ceremony".to_owned(),
                ceremony_id: Some("ceremony-a".to_owned()),
                ..AuthorizationScope::default()
            }),
            valid_from: Some(Timestamp {
                seconds: 1_700_000_000,
                nanos: 0,
            }),
            valid_until: None,
            delegation_depth: 0,
            parent_grant_id: None,
        })
        .await
        .unwrap()
        .into_inner();
    assert!(!issued.existing);

    let policy = client
        .get_authorization_policy(GetAuthorizationPolicyRequest {})
        .await
        .unwrap()
        .into_inner()
        .policy
        .unwrap();
    assert_eq!(policy.policy_id, "grpc-fixture");
    assert!(policy
        .grants
        .iter()
        .any(|grant| grant.grant_id == "reader-a"));

    let decisions = client
        .list_authorization_decisions(ListAuthorizationDecisionsRequest {
            after_decision_id: None,
            limit: 100,
        })
        .await
        .unwrap()
        .into_inner();
    assert!(decisions
        .decisions
        .iter()
        .any(|decision| decision.action == "issue_authorization_grant"));

    let revoked = client
        .revoke_authorization_grant(RevokeAuthorizationGrantRequest {
            grant_id: "reader-a".to_owned(),
            reason: "rotation complete".to_owned(),
        })
        .await
        .unwrap()
        .into_inner();
    assert!(!revoked.existing);

    let policy = client
        .get_authorization_policy(GetAuthorizationPolicyRequest {})
        .await
        .unwrap()
        .into_inner()
        .policy
        .unwrap();
    assert_eq!(policy.revoked_grant_ids, ["reader-a"]);
}
