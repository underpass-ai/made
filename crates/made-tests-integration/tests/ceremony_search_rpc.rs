use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::grpc::GrpcAuthorizationGate;
use made_adapters::memory::InMemoryAuthorizationPolicyStore;
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
};
use made_client::v1::{
    CeremonyLifecycleFilter, PauseCeremonyRequest, SearchCeremonyInstancesRequest,
    StartCeremonyRequest,
};
use made_client::{MadeClient, MadeClientError};
use made_core::ports::ClockPort;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionTtl,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId,
    AuthorizationScope, DelegationDepth, PrincipalId, PrincipalKind,
};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::ListCeremonyInstancesRequest;
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use tonic::metadata::MetadataValue;
use tonic::Code;
use tonic::Request;

const CEREMONY: &str = r#"
version: "1.0"
name: searchable
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
steps:
  - {id: work, state: OPEN, handler: host_callback}
roles:
  - {id: WORKER, allowed_actions: [work, finish]}
"#;

#[tokio::test]
async fn search_pages_authoritative_instances_and_rejects_cursor_rebinding() {
    let fixture = GrpcFixture::start().await;
    let endpoint = format!("http://{}", fixture.addr);
    let mut rpc = MadeServiceClient::new(fixture.channel);
    for (position, id) in ["search-a", "search-b", "search-c"].into_iter().enumerate() {
        let mut request = Request::new(StartCeremonyRequest {
            ceremony_id: id.to_owned(),
            definition_yaml: CEREMONY.to_owned(),
            context: None,
            actor_id: "search-test".to_owned(),
            actor_kind: "service".to_owned(),
        });
        request.metadata_mut().insert(
            "x-made-request-id",
            MetadataValue::try_from(format!("search-start-{position}")).unwrap(),
        );
        rpc.start_ceremony(request).await.unwrap();
    }

    let client = MadeClient::connect(endpoint).await.unwrap();
    client
        .pause(PauseCeremonyRequest {
            ceremony_id: "search-b".to_owned(),
            actor_id: "search-test".to_owned(),
            actor_kind: "service".to_owned(),
            reason: "inspect".to_owned(),
        })
        .await
        .unwrap();

    let first = client
        .search_ceremonies(query(None, 2, None))
        .await
        .unwrap();
    assert_eq!(
        ceremony_ids(first.instances()),
        vec!["search-a", "search-b"]
    );
    let cursor = first
        .next_cursor()
        .expect("three rows require another page");

    let second = client
        .search_ceremonies(query(Some(cursor), 2, None))
        .await
        .unwrap();
    assert_eq!(ceremony_ids(second.instances()), vec!["search-c"]);
    assert!(second.next_cursor().is_none());

    let paused = client
        .search_ceremonies(query(None, 10, Some(CeremonyLifecycleFilter::Paused)))
        .await
        .unwrap();
    assert_eq!(ceremony_ids(paused.instances()), vec!["search-b"]);

    let rebound = client
        .search_ceremonies(query(
            Some(cursor),
            2,
            Some(CeremonyLifecycleFilter::Paused),
        ))
        .await;
    assert!(matches!(rebound, Err(MadeClientError::RemoteStatus { .. })));

    let mut tampered = cursor.to_owned();
    tampered.push('x');
    let rejected = client
        .search_ceremonies(query(Some(&tampered), 2, None))
        .await;
    assert!(matches!(
        rejected,
        Err(MadeClientError::RemoteStatus { .. })
    ));
}

#[tokio::test]
async fn search_permission_does_not_authorize_the_legacy_listing() {
    let fixture = GrpcFixture::start_with(
        GrpcFixtureWiring::new().with_authorization(
            authorization([
                AuthorizationAction::StartCeremony,
                AuthorizationAction::SearchCeremonyInstances,
            ])
            .await,
        ),
    )
    .await;
    let mut rpc = MadeServiceClient::new(fixture.channel);
    rpc.start_ceremony(with_request_id(
        StartCeremonyRequest {
            ceremony_id: "search-permission".to_owned(),
            definition_yaml: CEREMONY.to_owned(),
            context: None,
            actor_id: "payload-actor".to_owned(),
            actor_kind: "service".to_owned(),
        },
        "search-permission-start",
    ))
    .await
    .unwrap();

    let page = rpc
        .search_ceremony_instances(with_request_id(
            query(None, 10, None),
            "search-permission-page",
        ))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ceremony_ids(&page.instances), vec!["search-permission"]);

    let denied = rpc
        .list_ceremony_instances(with_request_id(
            ListCeremonyInstancesRequest {},
            "legacy-listing-denied",
        ))
        .await
        .unwrap_err();
    assert_eq!(denied.code(), Code::PermissionDenied);
}

#[tokio::test]
async fn one_request_id_cannot_cross_from_search_to_listing() {
    let fixture = GrpcFixture::start_with(
        GrpcFixtureWiring::new().with_authorization(
            authorization([
                AuthorizationAction::SearchCeremonyInstances,
                AuthorizationAction::ListCeremonyInstances,
            ])
            .await,
        ),
    )
    .await;
    let mut rpc = MadeServiceClient::new(fixture.channel);
    rpc.search_ceremony_instances(with_request_id(query(None, 10, None), "shared-request"))
        .await
        .unwrap();
    let conflict = rpc
        .list_ceremony_instances(with_request_id(
            ListCeremonyInstancesRequest {},
            "shared-request",
        ))
        .await
        .unwrap_err();
    assert_eq!(conflict.code(), Code::Aborted);
}

async fn authorization(
    actions: impl IntoIterator<Item = AuthorizationAction>,
) -> Arc<GrpcAuthorizationGate> {
    let clock: Arc<dyn ClockPort> = Arc::new(SystemClock::new());
    let policy_id = AuthorizationPolicyId::new("search-specific-policy").unwrap();
    let principal = AuthenticatedPrincipal::new(
        PrincipalId::new("search-specific-host").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    let store = Arc::new(InMemoryAuthorizationPolicyStore::new());
    let administration = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
    );
    administration
        .open(principal.clone(), Vec::new())
        .await
        .unwrap();
    administration
        .issue(
            &principal,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("search-specific-grant").unwrap(),
                principal.id().clone(),
                actions,
                AuthorizationScope::Global,
                (clock.now(), None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(principal.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    Arc::new(
        GrpcAuthorizationGate::trusted_host(
            Arc::new(AuthorizeOperationUseCase::new(
                policy_id,
                store,
                clock,
                AuthorizationDecisionTtl::from_seconds(60).unwrap(),
            )),
            principal,
            "search-specific-policy",
        )
        .unwrap(),
    )
}

fn with_request_id<T>(message: T, request_id: &'static str) -> Request<T> {
    let mut request = Request::new(message);
    request
        .metadata_mut()
        .insert("x-made-request-id", MetadataValue::from_static(request_id));
    request
}

fn query(
    cursor: Option<&str>,
    limit: u32,
    lifecycle: Option<CeremonyLifecycleFilter>,
) -> SearchCeremonyInstancesRequest {
    SearchCeremonyInstancesRequest {
        cursor: cursor.unwrap_or_default().to_owned(),
        limit,
        id_prefix: "search-".to_owned(),
        lifecycle: lifecycle.unwrap_or(CeremonyLifecycleFilter::Unspecified) as i32,
    }
}

fn ceremony_ids(instances: &[made_client::v1::CeremonyInstanceState]) -> Vec<&str> {
    instances
        .iter()
        .map(|instance| instance.ceremony_id.as_str())
        .collect()
}
