use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::memory::InMemoryAuthorizationPolicyStore;
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
    TrustedHostAuthorizationGate,
};
use made_app::services::AuthorizationOperationScope;
use made_core::ports::CeremonyAgentStatusQuery;
use made_core::ports::ClockPort;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionTtl,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId,
    AuthorizationRequestId, AuthorizationScope, AuthorizationTargetDigest, CeremonyName,
    CeremonyVersion, DelegationDepth, PrincipalId, PrincipalKind, SeparationRule,
};
use made_embedded::EmbeddedMade;

const DEFINITION: &str = r#"
version: "1.0"
name: protected_direct
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
  - id: work
    state: OPEN
    handler: noop
roles:
  - id: OPERATOR
    allowed_actions: [work, finish]
"#;

#[tokio::test]
async fn direct_facade_requires_exact_typed_approval_action_and_scope() {
    let (engine, executor_gate, definition_scope) = fixture().await;

    let missing = engine.definitions().await.unwrap_err();
    assert!(missing.to_string().contains("authorized operation context"));

    mount_with_approval(&engine, &executor_gate, &definition_scope).await;

    let wrong_action = executor_gate
        .authorize(
            AuthorizationRequestId::new("direct-list-action").unwrap(),
            AuthorizationAction::ListCeremonyDefinitions,
            AuthorizationScope::Global,
            AuthorizationTargetDigest::for_bytes(b"list"),
            None,
        )
        .await
        .unwrap();
    let error = AuthorizationOperationScope::run(
        wrong_action,
        engine.definition(
            &CeremonyName::new("protected_direct").unwrap(),
            &CeremonyVersion::new("1.0").unwrap(),
        ),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("action does not match"));

    let exact_read = executor_gate
        .authorize(
            AuthorizationRequestId::new("direct-read").unwrap(),
            AuthorizationAction::GetCeremonyDefinition,
            definition_scope,
            AuthorizationTargetDigest::for_bytes(b"read"),
            None,
        )
        .await
        .unwrap();
    let neighbor = AuthorizationOperationScope::run(
        exact_read,
        engine.definition(
            &CeremonyName::new("neighbor").unwrap(),
            &CeremonyVersion::new("1.0").unwrap(),
        ),
    )
    .await
    .unwrap_err();
    assert!(neighbor
        .to_string()
        .contains("does not admit this definition"));
}

#[tokio::test]
async fn live_agent_reads_are_refused_without_their_own_authorization() {
    let (engine, executor_gate, _) = fixture().await;
    let query = CeremonyAgentStatusQuery::new("ceremony-live", None, 10, None).unwrap();

    let missing = engine.list_agents(query.clone()).await.unwrap_err();
    assert!(missing.to_string().contains("authorized operation context"));

    let unrelated = executor_gate
        .authorize(
            AuthorizationRequestId::new("agent-status-wrong-action").unwrap(),
            AuthorizationAction::ListCeremonyDefinitions,
            AuthorizationScope::Global,
            AuthorizationTargetDigest::for_bytes(b"agent-status"),
            None,
        )
        .await
        .unwrap();
    let wrong_action = AuthorizationOperationScope::run(unrelated, engine.list_agents(query))
        .await
        .unwrap_err();
    assert!(wrong_action.to_string().contains("action does not match"));
}

async fn fixture() -> (
    EmbeddedMade,
    TrustedHostAuthorizationGate,
    AuthorizationScope,
) {
    let clock = Arc::new(SystemClock::new());
    let policies = Arc::new(InMemoryAuthorizationPolicyStore::new());
    let policy_id = AuthorizationPolicyId::new("protected-direct").unwrap();
    let approver = principal("approver");
    let executor = principal("executor");
    let administration = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        policies.clone(),
        clock.clone(),
    );
    administration
        .open(
            approver.clone(),
            vec![SeparationRule::new(
                AuthorizationAction::ApproveCeremonyGuard,
                AuthorizationAction::MountDefinition,
            )
            .unwrap()],
        )
        .await
        .unwrap();
    let definition_scope = AuthorizationScope::Definition {
        name: CeremonyName::new("protected_direct").unwrap(),
        version: Some(CeremonyVersion::new("1.0").unwrap()),
    };
    for (id, grantee, actions) in [
        (
            "approval-grant",
            approver.id().clone(),
            vec![AuthorizationAction::ApproveCeremonyGuard],
        ),
        (
            "execution-grant",
            executor.id().clone(),
            vec![
                AuthorizationAction::MountDefinition,
                AuthorizationAction::GetCeremonyDefinition,
                AuthorizationAction::ListCeremonyDefinitions,
            ],
        ),
    ] {
        administration
            .issue(
                &approver,
                AuthorizationGrant::new(
                    AuthorizationGrantId::new(id).unwrap(),
                    grantee,
                    actions,
                    definition_scope.clone(),
                    (clock.now(), None),
                    DelegationDepth::none(),
                    AuthorizationGrantIssuer::direct(approver.clone()),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    administration
        .issue(
            &approver,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("global-list-grant").unwrap(),
                executor.id().clone(),
                vec![AuthorizationAction::ListCeremonyDefinitions],
                AuthorizationScope::Global,
                (clock.now(), None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(approver.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();

    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy_id.clone(),
        policies.clone(),
        clock,
        AuthorizationDecisionTtl::from_seconds(60).unwrap(),
    ));
    let approver_gate = TrustedHostAuthorizationGate::new(authorize.clone(), approver).unwrap();
    let executor_gate = TrustedHostAuthorizationGate::new(authorize, executor).unwrap();
    let engine = EmbeddedMade::builder()
        .with_authorization(Arc::new(approver_gate.clone()))
        .build()
        .with_authorization_policy(policy_id, policies);
    (engine, executor_gate, definition_scope)
}

async fn mount_with_approval(
    engine: &EmbeddedMade,
    executor_gate: &TrustedHostAuthorizationGate,
    definition_scope: &AuthorizationScope,
) {
    let target = AuthorizationTargetDigest::for_bytes(DEFINITION.as_bytes());
    let approval = engine
        .approve_authorization_operation(
            AuthorizationRequestId::new("direct-approval").unwrap(),
            AuthorizationAction::ApproveCeremonyGuard,
            AuthorizationAction::MountDefinition,
            definition_scope.clone(),
            target.clone(),
        )
        .await
        .unwrap();
    let operation = executor_gate
        .authorize(
            AuthorizationRequestId::new("direct-execution").unwrap(),
            AuthorizationAction::MountDefinition,
            definition_scope.clone(),
            target,
            Some(approval.id().clone()),
        )
        .await
        .unwrap();
    let definition = CeremonyDefinitionYaml::parse_str(DEFINITION).unwrap();
    AuthorizationOperationScope::run(operation, engine.mount_definition(definition))
        .await
        .unwrap();
}

fn principal(id: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new(id).unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap()
}
