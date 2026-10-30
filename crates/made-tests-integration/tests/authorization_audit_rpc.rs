use std::sync::Arc;

use made_adapters::artifacts::LocalArtifactStore;
use made_adapters::clock::SystemClock;
use made_adapters::grpc::GrpcAuthorizationGate;
use made_adapters::memory::{InMemoryAuthorizationPolicyStore, InMemoryProtocolEventStore};
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
};
use made_core::ports::{ArtifactPageLimit, ArtifactStorePort, ProtocolEventStorePort};
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionTtl,
    AuthorizationPolicyId, AuthorizationScope, PrincipalId, PrincipalKind, ProtocolEventPageLimit,
    ProtocolId, StreamVersion,
};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    ArtifactProvenance, ArtifactSourceKind, BeginArtifactUploadRequest, StartProtocolRequest,
};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use prost_types::Timestamp;
use tonic::Code;

const PROTOCOL: &str = include_str!("../../../tests/e2e/protocols/editorial-planning-meeting.yaml");

#[tokio::test]
async fn protected_rpc_seals_authorization_evidence_in_v3_records() {
    let store = Arc::new(InMemoryProtocolEventStore::new());
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_protocol_store(store.clone())).await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let protocol_id = ProtocolId::new("authorized-audit-rpc").unwrap();

    client
        .start_protocol(StartProtocolRequest {
            protocol_id: protocol_id.as_str().to_owned(),
            actor_id: "payload-is-not-the-principal".to_owned(),
            actor_kind: "service".to_owned(),
            definition_yaml: PROTOCOL.to_owned(),
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
            &protocol_id,
            StreamVersion::EMPTY,
            ProtocolEventPageLimit::new(32).unwrap(),
        )
        .await
        .unwrap();
    assert!(!records.is_empty());
    for record in records {
        assert_eq!(record.schema_version(), 3);
        let evidence = record
            .authorization_evidence()
            .expect("protected RPC facts must carry authorization evidence");
        assert_eq!(evidence.action(), AuthorizationAction::StartProtocol);
        assert!(matches!(
            evidence.scope(),
            AuthorizationScope::Protocol { protocol_id: sealed } if sealed == &protocol_id
        ));
        assert_eq!(evidence.principal_id().as_str(), "grpc-fixture-host");
    }
}

#[tokio::test]
async fn denied_rpc_leaves_the_protocol_journal_unchanged() {
    let store = Arc::new(InMemoryProtocolEventStore::new());
    let fixture = GrpcFixture::start_with(
        GrpcFixtureWiring::new()
            .with_protocol_store(store.clone())
            .with_authorization(deny_business_actions().await),
    )
    .await;
    let mut client = MadeServiceClient::new(fixture.channel);
    let protocol_id = ProtocolId::new("authorization-denied-no-append").unwrap();

    let status = client
        .start_protocol(StartProtocolRequest {
            protocol_id: protocol_id.as_str().to_owned(),
            actor_id: "denied-owner".to_owned(),
            actor_kind: "trusted_host".to_owned(),
            definition_yaml: PROTOCOL.to_owned(),
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
        store.head(&protocol_id).await.unwrap(),
        StreamVersion::EMPTY
    );
}

#[tokio::test]
async fn denied_artifact_rpc_leaves_the_repository_unchanged() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let store = Arc::new(LocalArtifactStore::open(directory.path()).unwrap());
    let fixture = GrpcFixture::start_with(
        GrpcFixtureWiring::new()
            .with_artifact_store(store.clone())
            .with_authorization(deny_business_actions().await),
    )
    .await;
    let mut client = MadeServiceClient::new(fixture.channel);

    let status =
        client
            .begin_artifact_upload(BeginArtifactUploadRequest {
                requested_artifact_id: None,
                expected_digest:
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_owned(),
                size_bytes: 1,
                media_type: "text/plain".to_owned(),
                provenance: Some(ArtifactProvenance {
                    source_kind: ArtifactSourceKind::GeneratedReport as i32,
                    execution_receipt_id: None,
                    operation_id: None,
                    accepted_claim_fence: None,
                    observed_at: Some(Timestamp {
                        seconds: 0,
                        nanos: 0,
                    }),
                    import_ref: None,
                }),
                idempotency_key: "denied-artifact-upload".to_owned(),
            })
            .await
            .unwrap_err();

    assert_eq!(status.code(), Code::PermissionDenied);
    assert!(store
        .list(None, ArtifactPageLimit::default())
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        std::fs::read_dir(directory.path().join("uploads"))
            .unwrap()
            .count(),
        0,
        "authorization denial must happen before the upload manifest is written"
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
