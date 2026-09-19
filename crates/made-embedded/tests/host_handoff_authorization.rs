use made_adapters::{memory::InMemoryAuthorizationPolicyStore, sqlite::SqliteCeremonyStore};
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
    TrustedHostAuthorizationGate,
};
use made_app::services::AuthorizationOperationScope;
use made_app::usecases::{StartCeremonyInput, StartCeremonyStepInput};
use made_app::workers::{InspectCeremonyResumeInput, RecordCeremonyHostHandoffInput};
use made_core::entities::ceremony_events::InstanceImported;
use made_core::entities::{AuditFact, CeremonyEvent, CeremonyInstance};
use made_core::ports::{CeremonyEventStorePort, ClockPort};
use made_core::value_objects::*;
use made_embedded::EmbeddedMade;
use std::sync::Arc;
use time::OffsetDateTime;

struct Clock;
impl ClockPort for Clock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH
    }
}
struct AtClock(OffsetDateTime);
impl ClockPort for AtClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}
fn id() -> CeremonyId {
    CeremonyId::new("protected-handoff").unwrap()
}
async fn admit(
    gate: &TrustedHostAuthorizationGate,
    action: AuthorizationAction,
    request: &str,
) -> AuthorizedOperation {
    admit_for(gate, action, request, id()).await
}
async fn admit_for(
    gate: &TrustedHostAuthorizationGate,
    action: AuthorizationAction,
    request: &str,
    ceremony_id: CeremonyId,
) -> AuthorizedOperation {
    gate.authorize(
        AuthorizationRequestId::new(request).unwrap(),
        action,
        AuthorizationScope::Ceremony { ceremony_id },
        AuthorizationTargetDigest::for_bytes(request.as_bytes()),
        None,
    )
    .await
    .unwrap()
}
fn principal(name: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new(name).unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap()
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // This authorization path must share one protected ceremony.
async fn ownership_scope_and_current_grants_protect_handoff_and_preflight() {
    let clock = Arc::new(Clock);
    let policies = Arc::new(InMemoryAuthorizationPolicyStore::new());
    let policy = AuthorizationPolicyId::new("handoff-auth").unwrap();
    let host = principal("host");
    let foreign = principal("foreign");
    let admin = AuthorizationPolicyAdministrationService::new(
        policy.clone(),
        policies.clone(),
        clock.clone(),
    );
    admin.open(host.clone(), vec![]).await.unwrap();
    for actor in [&host, &foreign] {
        admin
            .issue(
                &host,
                AuthorizationGrant::new(
                    AuthorizationGrantId::new(actor.id().as_str()).unwrap(),
                    actor.id().clone(),
                    [
                        AuthorizationAction::StartCeremony,
                        AuthorizationAction::ClaimCeremonyStep,
                        AuthorizationAction::RecordCeremonyHostHandoff,
                        AuthorizationAction::InspectCeremonyResume,
                    ],
                    AuthorizationScope::Global,
                    (clock.now(), None),
                    DelegationDepth::none(),
                    AuthorizationGrantIssuer::direct(host.clone()),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy.clone(),
        policies.clone(),
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(60).unwrap(),
    ));
    let gate = TrustedHostAuthorizationGate::new(authorize.clone(), host.clone()).unwrap();
    let foreign_gate = TrustedHostAuthorizationGate::new(authorize, foreign).unwrap();
    let engine = EmbeddedMade::builder().with_clock(clock).build();
    let definition = engine
        .mount_yaml(
            r#"
version: "1.0"
name: handoff_auth
states: [{id: WORK, initial: true}, {id: DONE, terminal: true}]
transitions: [{from: WORK, to: DONE, trigger: finish}]
steps: [{id: work, state: WORK, handler: host_callback}]
roles: [{id: WORKER, allowed_actions: [work, finish]}]
"#,
        )
        .await
        .unwrap()
        .definitions()[0]
        .clone();
    let engine = engine.with_authorization_policy(policy, policies);
    AuthorizationOperationScope::run(
        admit(&gate, AuthorizationAction::StartCeremony, "start").await,
        engine.start(StartCeremonyInput::new(
            id(),
            definition.name().clone(),
            definition.version().clone(),
            CeremonyContext::empty(),
            "host",
            AuditActorKind::Agent,
        )),
    )
    .await
    .unwrap();
    let claim = Box::pin(AuthorizationOperationScope::run(
        admit(&gate, AuthorizationAction::ClaimCeremonyStep, "claim").await,
        engine.start_step(StartCeremonyStepInput::new(
            id(),
            RoleId::new("WORKER").unwrap(),
            AuditActorKind::Agent,
            StepId::new("work").unwrap(),
            LeaseOwnerId::new("logical-owner").unwrap(),
            IdempotencyKey::new("claim").unwrap(),
            DurationMs::from_millis(60_000),
        )),
    ))
    .await
    .unwrap();
    let input = RecordCeremonyHostHandoffInput {
        ceremony_id: id(),
        declaration: HostHandoffDeclaration {
            id: IdempotencyKey::new("ack").unwrap(),
            step_id: StepId::new("work").unwrap(),
            claim_fence: claim.claim_fence().clone(),
            owner: LeaseOwnerId::new("logical-owner").unwrap(),
            incarnation: HostAgentIncarnation::new("task-1").unwrap(),
            state: HostWorkState::Quiesced,
            observed_at: OffsetDateTime::UNIX_EPOCH,
            evidence: EvidenceReference::new("artifact:evidence").unwrap(),
        },
    };
    let denied = AuthorizationOperationScope::run(
        admit(
            &foreign_gate,
            AuthorizationAction::RecordCeremonyHostHandoff,
            "foreign",
        )
        .await,
        engine.record_ceremony_host_handoff(input.clone()),
    )
    .await
    .unwrap_err();
    assert!(denied.to_string().contains("principal does not own"));
    let operation = admit(&gate, AuthorizationAction::RecordCeremonyHostHandoff, "ack").await;
    let accepted = AuthorizationOperationScope::run(
        operation.clone(),
        engine.record_ceremony_host_handoff(input.clone()),
    )
    .await
    .unwrap();
    let query = InspectCeremonyResumeInput {
        ceremony_id: id(),
        after_claim: None,
        limit: ExecutionRecoveryPageLimit::DEFAULT,
    };
    assert!(engine.inspect_ceremony_resume(query.clone()).await.is_err());
    let inspect = admit(&gate, AuthorizationAction::InspectCeremonyResume, "inspect").await;
    let report = AuthorizationOperationScope::run(
        inspect.clone(),
        engine.inspect_ceremony_resume(query.clone()),
    )
    .await
    .unwrap();
    assert!(report.all_claims_host_reported_quiesced);
    assert_eq!(report.claims[0].host_declaration.as_ref(), Some(&accepted));
    let mut wrong_scope = query.clone();
    wrong_scope.ceremony_id = CeremonyId::new("other").unwrap();
    assert!(AuthorizationOperationScope::run(
        inspect.clone(),
        engine.inspect_ceremony_resume(wrong_scope)
    )
    .await
    .is_err());
    admin
        .revoke(
            &host,
            &AuthorizationGrantId::new("host").unwrap(),
            AuthorizationRevocationReason::new("revoked").unwrap(),
        )
        .await
        .unwrap();
    assert!(AuthorizationOperationScope::run(
        operation,
        engine.record_ceremony_host_handoff(input)
    )
    .await
    .is_err());
    let unchanged =
        AuthorizationOperationScope::run(inspect, engine.inspect_ceremony_resume(query))
            .await
            .unwrap();
    assert_eq!(unchanged.journal_version, report.journal_version);
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // The imported and protected boundary is one security invariant.
async fn protected_legacy_import_binds_handoff_owner_to_the_authenticated_principal() {
    let imported_at = time::macros::datetime!(2026-07-29 09:10:00 UTC);
    let clock = Arc::new(AtClock(imported_at));
    let policies = Arc::new(InMemoryAuthorizationPolicyStore::new());
    let policy = AuthorizationPolicyId::new("legacy-handoff-auth").unwrap();
    let admin_principal = principal("admin");
    let legacy = principal("legacy-host");
    let foreign = principal("foreign-host");
    let admin = AuthorizationPolicyAdministrationService::new(
        policy.clone(),
        policies.clone(),
        clock.clone(),
    );
    admin.open(admin_principal.clone(), vec![]).await.unwrap();
    for actor in [&legacy, &foreign] {
        admin
            .issue(
                &admin_principal,
                AuthorizationGrant::new(
                    AuthorizationGrantId::new(format!("{}-handoff", actor.id().as_str())).unwrap(),
                    actor.id().clone(),
                    [AuthorizationAction::RecordCeremonyHostHandoff],
                    AuthorizationScope::Global,
                    (clock.now(), None),
                    DelegationDepth::none(),
                    AuthorizationGrantIssuer::direct(admin_principal.clone()),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy.clone(),
        policies.clone(),
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(60).unwrap(),
    ));
    let legacy_gate = TrustedHostAuthorizationGate::new(authorize.clone(), legacy).unwrap();
    let foreign_gate = TrustedHostAuthorizationGate::new(authorize, foreign).unwrap();

    let scratch =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/host-handoff-188");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let store =
        Arc::new(SqliteCeremonyStore::open(directory.path().join("legacy.sqlite3")).unwrap());
    let snapshot: CeremonyInstance = serde_json::from_str(include_str!(
        "../../made-core/tests/fixtures/legacy_in_progress_instance_pre_p5.json"
    ))
    .unwrap();
    let ceremony_id = snapshot.id().clone();
    store
        .append(
            &ceremony_id,
            StreamVersion::EMPTY,
            vec![AuditFact {
                event_id: EventId::new("protected-legacy:import:1").unwrap(),
                ceremony_id: ceremony_id.clone(),
                definition_name: snapshot.definition_name().clone(),
                definition_version: snapshot.definition_version().clone(),
                occurred_at: imported_at,
                actor: AuditActor::engine("made-migration").unwrap(),
                correlation_id: None,
                causation_id: None,
                trace: None,
                event: CeremonyEvent::InstanceImported(InstanceImported {
                    ceremony_id: ceremony_id.clone(),
                    definition_name: snapshot.definition_name().clone(),
                    definition_version: snapshot.definition_version().clone(),
                    snapshot: Box::new(snapshot),
                    legacy_journal_head_hash: None,
                    legacy_revision: CeremonyRevision::INITIAL,
                    imported_at,
                }),
            }],
        )
        .await
        .unwrap();
    let engine = EmbeddedMade::builder()
        .with_ceremony_store(store.clone())
        .with_clock(clock)
        .build();
    engine
        .mount_yaml(
            r#"
version: "1.0"
name: legacy_static
states: [{id: OPEN, initial: true}]
steps: [{id: draft, state: OPEN, handler: host_callback}]
roles: [{id: LEGACY, allowed_actions: [draft]}]
retry_policies: {default: {max_attempts: 3, backoff_seconds: 0}}
"#,
        )
        .await
        .unwrap();
    let engine = engine.with_authorization_policy(policy, policies);
    let _report = engine
        .inspect_ceremony_resume(InspectCeremonyResumeInput {
            ceremony_id: ceremony_id.clone(),
            after_claim: None,
            limit: ExecutionRecoveryPageLimit::DEFAULT,
        })
        .await
        .unwrap_err();
    let fence = store
        .read(
            &ceremony_id,
            StreamVersion::EMPTY,
            CeremonyEventPageLimit::new(1).unwrap(),
        )
        .await
        .unwrap();
    let CeremonyEvent::InstanceImported(imported) = fence[0].event().unwrap() else {
        unreachable!()
    };
    let fence = imported
        .snapshot
        .step_claim_fence(&StepId::new("draft").unwrap())
        .unwrap();
    let input = RecordCeremonyHostHandoffInput {
        ceremony_id: ceremony_id.clone(),
        declaration: HostHandoffDeclaration {
            id: IdempotencyKey::new("legacy-protected-ack").unwrap(),
            step_id: StepId::new("draft").unwrap(),
            claim_fence: fence.clone(),
            owner: LeaseOwnerId::new("legacy-host").unwrap(),
            incarnation: HostAgentIncarnation::new("legacy-task").unwrap(),
            state: HostWorkState::Quiesced,
            observed_at: imported_at,
            evidence: EvidenceReference::new("artifact:legacy-auth").unwrap(),
        },
    };
    for denied in [
        AuthorizationOperationScope::run(
            admit_for(
                &foreign_gate,
                AuthorizationAction::RecordCeremonyHostHandoff,
                "foreign",
                ceremony_id.clone(),
            )
            .await,
            engine.record_ceremony_host_handoff(input.clone()),
        )
        .await,
        {
            let mut wrong_owner = input.clone();
            wrong_owner.declaration.owner = LeaseOwnerId::new("other-owner").unwrap();
            AuthorizationOperationScope::run(
                admit_for(
                    &legacy_gate,
                    AuthorizationAction::RecordCeremonyHostHandoff,
                    "wrong-owner",
                    ceremony_id.clone(),
                )
                .await,
                engine.record_ceremony_host_handoff(wrong_owner),
            )
            .await
        },
        {
            let mut wrong_fence = input.clone();
            wrong_fence.declaration.claim_fence = StepClaimFence::new("0".repeat(64)).unwrap();
            AuthorizationOperationScope::run(
                admit_for(
                    &legacy_gate,
                    AuthorizationAction::RecordCeremonyHostHandoff,
                    "wrong-fence",
                    ceremony_id.clone(),
                )
                .await,
                engine.record_ceremony_host_handoff(wrong_fence),
            )
            .await
        },
    ] {
        assert!(denied.is_err());
        assert_eq!(
            store
                .read(
                    &ceremony_id,
                    StreamVersion::EMPTY,
                    CeremonyEventPageLimit::new(100).unwrap(),
                )
                .await
                .unwrap()
                .len(),
            1
        );
    }
    let recorded = AuthorizationOperationScope::run(
        admit_for(
            &legacy_gate,
            AuthorizationAction::RecordCeremonyHostHandoff,
            "legacy-ok",
            ceremony_id.clone(),
        )
        .await,
        engine.record_ceremony_host_handoff(input),
    )
    .await
    .unwrap();
    assert_eq!(recorded.declaration.claim_fence, fence);
    let reopened = EmbeddedMade::builder().with_ceremony_store(store).build();
    assert_eq!(
        reopened
            .audit_records(&ceremony_id)
            .await
            .unwrap()
            .last()
            .unwrap()
            .event_type(),
        AuditEventType::HostHandoffRecorded
    );
}
