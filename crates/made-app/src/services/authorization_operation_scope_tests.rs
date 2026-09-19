use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationEvidence, AuthorizedOperation,
    PrincipalId, PrincipalKind,
};

use super::{current_authorized_operation, AuthorizationOperationScope, SessionStream};

#[tokio::test]
async fn concurrent_authorized_operations_keep_their_own_evidence() {
    let first = operation("first");
    let second = operation("second");

    let (first_seen, second_seen) = tokio::join!(
        AuthorizationOperationScope::run(first, async {
            tokio::task::yield_now().await;
            current_authorized_operation().unwrap()
        }),
        AuthorizationOperationScope::run(second, async {
            tokio::task::yield_now().await;
            current_authorized_operation().unwrap()
        })
    );

    assert_eq!(first_seen.principal().id().as_str(), "first");
    assert_eq!(second_seen.principal().id().as_str(), "second");
}

#[tokio::test]
async fn spawned_work_loses_the_operation_and_protected_append_fails_closed() {
    AuthorizationOperationScope::run(operation("parent"), async {
        let result = tokio::spawn(async {
            assert!(current_authorized_operation().is_none());
            SessionStream::active_authorization(true)
        })
        .await
        .unwrap();

        assert!(matches!(
            result,
            Err(made_core::DomainError::InvariantViolated {
                reason: "protected ceremony append has no active authorization"
            })
        ));
    })
    .await;
}

fn operation(id: &str) -> AuthorizedOperation {
    let principal = AuthenticatedPrincipal::new(
        PrincipalId::new(id).unwrap(),
        PrincipalKind::Worker,
        AuthenticationMethod::MutualTls,
    )
    .unwrap();
    let evidence: AuthorizationEvidence = serde_json::from_value(serde_json::json!({
        "decision_id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "request_id": format!("request-{id}"),
        "principal_id": id,
        "action": "complete_ceremony_step",
        "scope": { "kind": "ceremony", "ceremony_id": "ceremony-1" },
        "target_digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "policy_version": 1,
        "admitted_at": "2026-09-19T12:00:00Z",
        "valid_until": "2026-09-19T12:01:00Z"
    }))
    .unwrap();
    AuthorizedOperation::new(principal, evidence).unwrap()
}
