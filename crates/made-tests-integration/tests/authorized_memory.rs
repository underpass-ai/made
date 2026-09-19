//! A shared memory key is a search selector, never authority over its sources.
#[path = "authorized_memory/fixture.rs"]
mod fixture;

use fixture::{entry, relation, Fixture};
use made_app::services::AuthorizationOperationScope;
use made_core::ports::{AuthorizationPolicyStorePort, MemoryReaderPort, MemoryWriterPort};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationDecisionKind, AuthorizationDecisionPageLimit,
    AuthorizationGrantId, AuthorizationRevocationReason, AuthorizationScope, CeremonyId,
    MemoryEntryId, MemoryMoment, MemoryWrite,
};
use made_tests_integration::parity_clock::PARITY_INSTANT;
use time::Duration;

#[tokio::test]
async fn no_authenticated_operation_cannot_read_any_memory_surface() {
    let fixture = Fixture::new().await;
    assert!(fixture.reader.recall(&fixture.scope).await.is_err());
    assert!(fixture
        .reader
        .as_known_at(&fixture.scope, MemoryMoment::at(PARITY_INSTANT))
        .await
        .is_err());
    assert!(fixture
        .reader
        .follow(
            &fixture.scope,
            &MemoryEntryId::new("a1").unwrap(),
            &MemoryEntryId::new("b1").unwrap()
        )
        .await
        .is_err());
}

#[tokio::test]
async fn starting_in_shared_scope_does_not_disclose_foreign_decisions_or_reasons() {
    let fixture = Fixture::new().await;
    let operation = fixture.operation("no-source-grants").await;
    let recalled =
        AuthorizationOperationScope::run(operation, fixture.reader.recall(&fixture.scope))
            .await
            .unwrap();
    assert!(recalled.entries().is_empty());
    assert!(recalled.relations().is_empty());
    let decisions = fixture
        .policies
        .decisions(
            &fixture.policy_id,
            None,
            AuthorizationDecisionPageLimit::new(20).unwrap(),
        )
        .await
        .unwrap();
    let source_reads: Vec<_> = decisions
        .decisions()
        .iter()
        .filter(|d| d.request().action() == AuthorizationAction::ReadCeremonyEvents)
        .collect();
    assert_eq!(
        source_reads.len(),
        2,
        "one recorded admission per distinct source"
    );
    assert!(source_reads
        .iter()
        .all(|d| d.kind() == AuthorizationDecisionKind::Deny));
    assert_eq!(
        fixture
            .memory
            .recall(&fixture.scope)
            .await
            .unwrap()
            .entries()
            .len(),
        3
    );
}

#[tokio::test]
async fn exact_source_grant_discloses_only_its_entries_and_internal_reasons() {
    let fixture = Fixture::new().await;
    fixture.read_grant("read-a", "source-a").await;
    let operation = fixture.operation("read-a-only").await;
    let read =
        AuthorizationOperationScope::run(operation.clone(), fixture.reader.recall(&fixture.scope))
            .await
            .unwrap();
    assert_eq!(
        read.entries()
            .iter()
            .map(|e| e.id().as_str())
            .collect::<Vec<_>>(),
        ["a1", "a2"]
    );
    assert_eq!(
        read.relations(),
        &[relation("a1", "a2", "visible explanation")]
    );
    let version = fixture
        .policies
        .load(&fixture.policy_id)
        .await
        .unwrap()
        .unwrap()
        .version;
    let retried =
        AuthorizationOperationScope::run(operation, fixture.reader.recall(&fixture.scope))
            .await
            .unwrap();
    assert_eq!(read, retried);
    assert_eq!(
        fixture
            .policies
            .load(&fixture.policy_id)
            .await
            .unwrap()
            .unwrap()
            .version,
        version,
        "exact retry must not append duplicate decisions"
    );
    let path = AuthorizationOperationScope::run(
        fixture.operation("follow-a").await,
        fixture.reader.follow(
            &fixture.scope,
            &MemoryEntryId::new("a1").unwrap(),
            &MemoryEntryId::new("b1").unwrap(),
        ),
    )
    .await
    .unwrap();
    assert!(path.entries().is_empty());
    assert!(
        path.relations().is_empty(),
        "partial path leaks the hidden endpoint"
    );
    let visible_path = AuthorizationOperationScope::run(
        fixture.operation("follow-visible").await,
        fixture.reader.follow(
            &fixture.scope,
            &MemoryEntryId::new("a1").unwrap(),
            &MemoryEntryId::new("a2").unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        visible_path.relations(),
        &[relation("a1", "a2", "visible explanation")]
    );
}

#[tokio::test]
async fn historical_queries_use_present_permissions_after_revocation() {
    let fixture = Fixture::new().await;
    fixture.read_grant("read-a", "source-a").await;
    let before = AuthorizationOperationScope::run(
        fixture.operation("before-revoke").await,
        fixture
            .reader
            .as_known_at(&fixture.scope, MemoryMoment::at(PARITY_INSTANT)),
    )
    .await
    .unwrap();
    assert_eq!(before.entries().len(), 2);
    fixture
        .admin
        .revoke(
            &fixture.owner,
            &AuthorizationGrantId::new("read-a").unwrap(),
            AuthorizationRevocationReason::new("access withdrawn").unwrap(),
        )
        .await
        .unwrap();
    let after = AuthorizationOperationScope::run(
        fixture.operation("after-revoke").await,
        fixture
            .reader
            .as_known_at(&fixture.scope, MemoryMoment::at(PARITY_INSTANT)),
    )
    .await
    .unwrap();
    assert!(after.entries().is_empty());
    assert!(after.relations().is_empty());
    let path = AuthorizationOperationScope::run(
        fixture.operation("follow-after-revoke").await,
        fixture.reader.follow(
            &fixture.scope,
            &MemoryEntryId::new("a1").unwrap(),
            &MemoryEntryId::new("b1").unwrap(),
        ),
    )
    .await
    .unwrap();
    assert!(path.relations().is_empty());
}

#[tokio::test]
async fn expired_grants_and_unknown_tree_lineage_do_not_authorize_sources() {
    let fixture = Fixture::new().await;
    fixture
        .issue(
            "expired",
            AuthorizationAction::ReadCeremonyEvents,
            AuthorizationScope::Ceremony {
                ceremony_id: CeremonyId::new("source-a").unwrap(),
            },
            Some(PARITY_INSTANT - Duration::seconds(1)),
        )
        .await;
    fixture
        .issue(
            "tree",
            AuthorizationAction::ReadCeremonyEvents,
            AuthorizationScope::CeremonyTree {
                root_id: CeremonyId::new("unresolved-root").unwrap(),
            },
            None,
        )
        .await;
    let read = AuthorizationOperationScope::run(
        fixture.operation("expired-and-unknown").await,
        fixture.reader.recall(&fixture.scope),
    )
    .await
    .unwrap();
    assert!(read.entries().is_empty());
    assert!(read.relations().is_empty());
}

#[tokio::test]
async fn colliding_ids_with_different_sources_cannot_launder_private_relations() {
    let fixture = Fixture::new().await;
    fixture.read_grant("read-a", "source-a").await;
    fixture
        .memory
        .remember(
            &fixture.scope,
            MemoryWrite::new(
                vec![entry("b1", "source-a")],
                vec![relation("b1", "a1", "hidden reason written by source-b")],
            )
            .unwrap(),
            "ambiguous-provenance",
        )
        .await
        .unwrap();
    let read = AuthorizationOperationScope::run(
        fixture.operation("collision").await,
        fixture.reader.recall(&fixture.scope),
    )
    .await
    .unwrap();
    assert_eq!(read.entries().len(), 2);
    assert!(read.entries().iter().all(|e| e.id().as_str() != "b1"));
    assert_eq!(
        read.relations(),
        &[relation("a1", "a2", "visible explanation")]
    );
}
