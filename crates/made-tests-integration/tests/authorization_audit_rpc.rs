use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::grpc::GrpcAuthorizationGate;
use made_adapters::memory::{InMemoryAuthorizationPolicyStore, InMemoryCeremonyEventStore};
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
};
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionTtl,
    AuthorizationPolicyId, AuthorizationScope, CeremonyEventPageLimit, CeremonyId, PrincipalId,
    PrincipalKind, StreamVersion,
};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::StartCeremonyRequest;
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use tonic::Code;

const CEREMONY: &str =
    include_str!("../../../tests/e2e/ceremonies/editorial-planning-meeting.yaml");

#[tokio::test]
async fn protected_rpc_seals_authorization_evidence_in_v3_records() {
    let store = Arc::new(InMemoryCeremonyEventStore::new());
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_ceremony_store(store.clone())).await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = CeremonyId::new("authorized-audit-rpc").unwrap();

    client
        .start_ceremony(StartCeremonyRequest {
            ceremony_id: ceremony_id.as_str().to_owned(),
            actor_id: "payload-is-not-the-principal".to_owned(),
            actor_kind: "service".to_owned(),
            definition_yaml: CEREMONY.to_owned(),
            context: Some(prost_types::Struct {
                fields: [(
                    "meeting_brief".to_owned(),
                    prost_types::Value {
                        kind: Some(prost_types::value::Kind::StringValue(
                            "authorization evidence".to_owned(),
                        )),
                    },
                )]
                .into_iter()
                .collect(),
            }),
        })
        .await
        .unwrap();

    let records = store
        .read(
            &ceremony_id,
            StreamVersion::EMPTY,
            CeremonyEventPageLimit::new(32).unwrap(),
        )
        .await
        .unwrap();
    assert!(!records.is_empty());
    for record in records {
        assert_eq!(record.schema_version(), 3);
        let evidence = record
            .authorization_evidence()
            .expect("protected RPC facts must carry authorization evidence");
        assert_eq!(evidence.action(), AuthorizationAction::StartCeremony);
        assert!(matches!(
            evidence.scope(),
            AuthorizationScope::Ceremony { ceremony_id: sealed } if sealed == &ceremony_id
        ));
        assert_eq!(evidence.principal_id().as_str(), "grpc-fixture-host");
    }
}

#[tokio::test]
async fn denied_rpc_leaves_the_ceremony_journal_unchanged() {
    let store = Arc::new(InMemoryCeremonyEventStore::new());
    let fixture = GrpcFixture::start_with(
        GrpcFixtureWiring::new()
            .with_ceremony_store(store.clone())
            .with_authorization(deny_business_actions().await),
    )
    .await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let ceremony_id = CeremonyId::new("authorization-denied-no-append").unwrap();

    let status = client
        .start_ceremony(StartCeremonyRequest {
            ceremony_id: ceremony_id.as_str().to_owned(),
            actor_id: "denied-owner".to_owned(),
            actor_kind: "trusted_host".to_owned(),
            definition_yaml: CEREMONY.to_owned(),
            context: Some(prost_types::Struct {
                fields: [(
                    "meeting_brief".to_owned(),
                    prost_types::Value {
                        kind: Some(prost_types::value::Kind::StringValue("denied".to_owned())),
                    },
                )]
                .into_iter()
                .collect(),
            }),
        })
        .await
        .unwrap_err();

    assert_eq!(status.code(), Code::PermissionDenied);
    assert_eq!(
        store.head(&ceremony_id).await.unwrap(),
        StreamVersion::EMPTY
    );
}

async fn deny_business_actions() -> Arc<GrpcAuthorizationGate> {
    let clock = Arc::new(SystemClock::new());
    let policy_id = AuthorizationPolicyId::new("deny-business-actions").unwrap();
    let principal = AuthenticatedPrincipal::new(
        PrincipalId::new("denied-owner").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    let store = Arc::new(InMemoryAuthorizationPolicyStore::new());
    AuthorizationPolicyAdministrationService::new(policy_id.clone(), store.clone(), clock.clone())
        .open(principal.clone(), Vec::new())
        .await
        .unwrap();
    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy_id,
        store,
        clock,
        AuthorizationDecisionTtl::from_seconds(60).unwrap(),
    ));
    Arc::new(
        GrpcAuthorizationGate::trusted_host(authorize, principal, "deny-business-actions").unwrap(),
    )
}
