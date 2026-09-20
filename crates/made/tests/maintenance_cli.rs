use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;

use made_adapters::artifacts::LocalArtifactStore;
use made_adapters::clock::SystemClock;
use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_app::artifacts::ArtifactService;
use made_app::authorization::AuthorizationPolicyAdministrationService;
use made_core::ports::{ArtifactIdempotencyKey, ArtifactStorePort};
use made_core::value_objects::{
    ArtifactMediaType, AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId,
    AuthorizationScope, DelegationDepth, PrincipalId, PrincipalKind,
};
use serde_json::json;
use tempfile::TempDir;
use time::OffsetDateTime;

fn run(root: &Path, mode: &str, request: &serde_json::Value) -> Output {
    let input = root.join("request.json");
    std::fs::write(&input, serde_json::to_vec(request).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_made"))
        .env_remove("MADE_POSTGRES_URL")
        .env("MADE_CEREMONY_STORE_PATH", root.join("source.sqlite3"))
        .env("MADE_ARTIFACT_STORE_PATH", root.join("artifacts"))
        .env("MADE_AUTH_POLICY_ID", "test-policy")
        .env("MADE_AUTH_TRUSTED_HOST_ID", "test-host")
        .args(["maintenance", mode])
        .arg(input)
        .output()
        .unwrap()
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One CLI acceptance flow proves authorization, backup and restore together.
async fn maintenance_backup_requires_grant_and_emits_durable_authorization() {
    std::fs::create_dir_all("tmp").unwrap();
    let directory = TempDir::new_in("tmp").unwrap();
    let root = directory.path().canonicalize().unwrap();
    let store =
        Arc::new(SqliteAuthorizationPolicyStore::open(root.join("source.sqlite3")).unwrap());
    let administration = AuthorizationPolicyAdministrationService::new(
        AuthorizationPolicyId::new("test-policy").unwrap(),
        store,
        Arc::new(SystemClock::new()),
    );
    let owner = AuthenticatedPrincipal::new(
        PrincipalId::new("test-host").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    administration.open(owner.clone(), vec![]).await.unwrap();
    let artifacts = Arc::new(LocalArtifactStore::open(root.join("artifacts")).unwrap());
    let artifact = ArtifactService::new(artifacts)
        .save_generated_report(
            b"backup acceptance",
            ArtifactMediaType::new("text/plain").unwrap(),
            OffsetDateTime::now_utc(),
            ArtifactIdempotencyKey::new("input").unwrap(),
        )
        .await
        .unwrap();
    let request = json!({"command":{"operation":"backup", "destination":root.join("backup"), "key":"cli-backup"}});
    let described = run(&root, "--describe", &request);
    assert!(
        described.status.success(),
        "{}",
        String::from_utf8_lossy(&described.stderr)
    );
    assert!(!root.join("backup").exists());
    let denied = run(&root, "--request", &request);
    assert!(!denied.status.success());
    assert!(!root.join("backup").exists());
    let grant = AuthorizationGrant::new(
        AuthorizationGrantId::new("backup-only").unwrap(),
        owner.id().clone(),
        [AuthorizationAction::BackupStore],
        AuthorizationScope::Global,
        (OffsetDateTime::UNIX_EPOCH, None),
        DelegationDepth::none(),
        AuthorizationGrantIssuer::direct(owner.clone()),
    )
    .unwrap();
    administration.issue(&owner, grant).await.unwrap();
    let allowed = run(&root, "--request", &request);
    assert!(
        allowed.status.success(),
        "{}",
        String::from_utf8_lossy(&allowed.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&allowed.stdout).unwrap();
    assert_eq!(result["authorization"]["principal_id"], "test-host");
    assert_eq!(result["authorization"]["action"], "backup_store");
    assert!(root.join("backup/backup-set.json").is_file());
    let gc_preview = json!({"command":{
        "operation":"gc_preview", "retired_before":"1970-01-01T00:00:00Z"
    }});
    assert!(
        !run(&root, "--request", &gc_preview).status.success(),
        "backup permission must not authorize GC planning"
    );
    administration
        .issue(
            &owner,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("gc-preview-only").unwrap(),
                owner.id().clone(),
                [AuthorizationAction::GarbageCollectArtifacts],
                AuthorizationScope::Global,
                (OffsetDateTime::UNIX_EPOCH, None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(owner.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let gc_allowed = run(&root, "--request", &gc_preview);
    assert!(
        gc_allowed.status.success(),
        "{}",
        String::from_utf8_lossy(&gc_allowed.stderr)
    );
    let gc_result: serde_json::Value = serde_json::from_slice(&gc_allowed.stdout).unwrap();
    assert_eq!(
        gc_result["authorization"]["action"],
        "garbage_collect_artifacts"
    );
    let unrelated = run(
        &root,
        "--request",
        &json!({"command":{
            "operation":"release_protection", "key":"cli-backup", "reason":"should be denied"
        }}),
    );
    assert!(
        !unrelated.status.success(),
        "backup permission must not authorize release"
    );
    let restore = json!({"command":{"operation":"restore", "source":root.join("backup"), "destination":root.join("restored")}});
    assert!(!run(&root, "--request", &restore).status.success());
    assert!(!root.join("restored").exists());
    administration
        .issue(
            &owner,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("restore-only").unwrap(),
                owner.id().clone(),
                [AuthorizationAction::RestoreStore],
                AuthorizationScope::Global,
                (OffsetDateTime::UNIX_EPOCH, None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(owner.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let restored = run(&root, "--request", &restore);
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert!(root.join("restored/database.sqlite3").is_file());
    let restored_store = LocalArtifactStore::open(root.join("restored/artifacts")).unwrap();
    assert_eq!(
        restored_store
            .get(artifact.artifact_id())
            .await
            .unwrap()
            .artifact,
        artifact
    );
    assert!(restored_store
        .backup_content_available(artifact.artifact_id())
        .await
        .unwrap());
    std::fs::write(root.join("restored/sentinel"), "preserve").unwrap();
    assert!(!run(&root, "--request", &restore).status.success());
    assert_eq!(
        std::fs::read_to_string(root.join("restored/sentinel")).unwrap(),
        "preserve"
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One linear flow proves the remote effect occurs once across reconciliation retries.
async fn queryable_http_ambiguity_requires_marker_grant_and_never_replays_effect() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::Router;
    use made_adapters::connectors::HttpExecutionConnector;
    use made_adapters::sqlite::SqliteCeremonyStore;
    use made_app::workers::{RecoverExecutionIntentOutcome, RecoverExecutionIntentUseCase};
    use made_core::ports::{
        ArtifactByteOffset, BeginArtifactUpload, ExecutionReceiptStorePort, PutArtifactChunk,
    };
    use made_core::value_objects::{
        ArtifactDigest, ArtifactProvenance, ArtifactSizeBytes, ArtifactSourceKind, AuditActorKind,
        CeremonyId, ExecutionConnectorId, ExecutionIntent, ExecutionOperation, ExecutionReceipt,
        ExecutionReceiptId, ExecutionRecoveryCapability, ExecutionRequestBytes, StateIteration,
        StateVisit, StepClaimFence, StepId, StepIteration, StepOutput, StepResult,
    };
    use sha2::{Digest, Sha256};
    std::fs::create_dir_all("tmp").unwrap();
    let directory = TempDir::new_in("tmp").unwrap();
    let root = directory.path().canonicalize().unwrap();
    let policies =
        Arc::new(SqliteAuthorizationPolicyStore::open(root.join("source.sqlite3")).unwrap());
    let administration = AuthorizationPolicyAdministrationService::new(
        AuthorizationPolicyId::new("test-policy").unwrap(),
        policies,
        Arc::new(SystemClock::new()),
    );
    let owner = AuthenticatedPrincipal::new(
        PrincipalId::new("test-host").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    administration.open(owner.clone(), vec![]).await.unwrap();
    let store = Arc::new(SqliteCeremonyStore::open(root.join("source.sqlite3")).unwrap());
    let operation = ExecutionOperation::new(
        CeremonyId::new("ambiguous").unwrap(),
        StepId::new("external").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
        ExecutionRequestBytes::new(b"external effect".to_vec()).unwrap(),
    );
    let fence = StepClaimFence::new("3".repeat(64)).unwrap();
    let connector = ExecutionConnectorId::new("http").unwrap();
    let intent = ExecutionIntent::new(
        operation.clone(),
        fence.clone(),
        connector.clone(),
        ExecutionRecoveryCapability::QueryableByOperationId,
        ArtifactSourceKind::ExternalExecution,
        AuditActorKind::Engine,
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    store.record_intent(intent.clone()).await.unwrap();
    let artifacts = LocalArtifactStore::open(root.join("artifacts")).unwrap();
    let bytes = b"operator checked remote effect exactly once";
    let digest = ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(bytes))).unwrap();
    let upload = artifacts
        .begin_upload(BeginArtifactUpload {
            requested_artifact_id: None,
            expected_digest: digest.clone(),
            size_bytes: ArtifactSizeBytes::new(bytes.len() as u64),
            media_type: ArtifactMediaType::new("text/plain").unwrap(),
            provenance: ArtifactProvenance::execution(
                ArtifactSourceKind::ExternalExecution,
                ExecutionReceiptId::for_operation(operation.operation_id()),
                operation.operation_id().clone(),
                fence.clone(),
                OffsetDateTime::UNIX_EPOCH,
            )
            .unwrap(),
            idempotency_key: ArtifactIdempotencyKey::new("operator-proof").unwrap(),
        })
        .await
        .unwrap();
    artifacts
        .put_chunk(PutArtifactChunk {
            upload_id: upload.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            chunk_digest: digest,
            bytes: bytes.to_vec(),
        })
        .await
        .unwrap();
    let evidence = artifacts.commit_upload(&upload.upload_id).await.unwrap();
    let receipt = ExecutionReceipt::new(
        operation.operation_id().clone(),
        operation.request_digest().clone(),
        fence.clone(),
        connector.clone(),
        None,
        ExecutionRecoveryCapability::QueryableByOperationId,
        ArtifactSourceKind::ExternalExecution,
        StepResult::completed(StepOutput::empty()).unwrap(),
        vec![evidence],
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    let request = json!({"command":{"operation":"reconcile_execution","receipt":receipt}});
    administration
        .issue(
            &owner,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("backup-cannot-reconcile").unwrap(),
                owner.id().clone(),
                [AuthorizationAction::BackupStore],
                AuthorizationScope::Global,
                (OffsetDateTime::UNIX_EPOCH, None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(owner.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(!run(&root, "--request", &request).status.success());
    assert!(store
        .receipt(operation.operation_id())
        .await
        .unwrap()
        .is_none());
    administration
        .issue(
            &owner,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("reconcile-only").unwrap(),
                owner.id().clone(),
                [AuthorizationAction::ReconcileExecutionOperation],
                AuthorizationScope::Global,
                (OffsetDateTime::UNIX_EPOCH, None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(owner.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();

    // A capability alone is not evidence that the effect became ambiguous.
    // Even an authorized operator cannot reconcile before the connector path
    // durably records its unresolved outcome.
    assert!(!run(&root, "--request", &request).status.success());
    assert!(store
        .reconciliation_requirement(operation.operation_id(), &fence)
        .await
        .unwrap()
        .is_none());

    let gets = Arc::new(AtomicUsize::new(0));
    let observed_gets = gets.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let remote = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/operations/:key",
                get(move || {
                    observed_gets.fetch_add(1, Ordering::SeqCst);
                    async { StatusCode::SERVICE_UNAVAILABLE }
                }),
            ),
        )
        .await
        .unwrap();
    });
    let http = Arc::new(
        HttpExecutionConnector::new(
            connector.clone(),
            &base,
            root.join("http-operations"),
            std::time::Duration::from_secs(2),
        )
        .unwrap(),
    );
    assert!(matches!(
        RecoverExecutionIntentUseCase::new(store.clone(), http)
            .execute(&intent)
            .await
            .unwrap(),
        RecoverExecutionIntentOutcome::ReconciliationRequired(_)
    ));
    assert!(store
        .reconciliation_requirement(operation.operation_id(), &fence)
        .await
        .unwrap()
        .is_some());
    let mut authorization_audits = Vec::new();
    for duplicate in [false, true] {
        let output = run(&root, "--request", &request);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(response["result"]["already_recorded"], duplicate);
        assert_eq!(
            response["authorization"]["action"],
            "reconcile_execution_operation"
        );
        let audit: made_core::value_objects::ArtifactRef =
            serde_json::from_value(response["result"]["authorization_audit"].clone()).unwrap();
        authorization_audits.push(audit);
        assert!(artifacts
            .backup_content_available(authorization_audits.last().unwrap().artifact_id())
            .await
            .unwrap());
    }
    let audit = authorization_audits.first().unwrap();
    let audit_bytes = artifacts
        .read_chunk(made_core::ports::ReadArtifactChunk {
            artifact_id: audit.artifact_id().clone(),
            offset: made_core::ports::ArtifactByteOffset::ZERO,
            max_bytes: made_core::ports::ArtifactChunkLimit::MAX,
        })
        .await
        .unwrap();
    assert!(audit_bytes.is_complete());
    let audit: serde_json::Value = serde_json::from_slice(&audit_bytes.bytes).unwrap();
    assert_eq!(audit["phase"], "authorized_attempt");
    assert_eq!(audit["authorization"]["principal_id"], "test-host");
    assert_eq!(
        audit["authorization"]["action"],
        "reconcile_execution_operation"
    );
    assert_eq!(
        audit["receipt"]["operation_id"],
        operation.operation_id().as_str()
    );
    assert_eq!(
        audit["receipt"]["request_digest"],
        operation.request_digest().as_str()
    );
    assert_eq!(audit["receipt"]["connector_id"], connector.as_str());
    assert_eq!(audit["receipt"]["producer_claim_fence"], fence.as_str());
    assert_eq!(
        store.receipt(operation.operation_id()).await.unwrap(),
        Some(receipt)
    );
    assert_eq!(
        store.intents(operation.operation_id()).await.unwrap().len(),
        1
    );
    assert!(store
        .reconciliation_requirement(operation.operation_id(), &fence)
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        gets.load(Ordering::SeqCst),
        1,
        "reconciliation replayed HTTP"
    );
    remote.abort();
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Linear crash-to-authorized-reconciliation acceptance flow.
async fn non_queryable_crash_is_marked_without_replay_and_reconciled_by_authorized_cli() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use made_adapters::sqlite::SqliteCeremonyStore;
    use made_app::workers::{RecoverExecutionIntentOutcome, RecoverExecutionIntentUseCase};
    use made_core::ports::{
        ArtifactByteOffset, BeginArtifactUpload, CeremonyExecutionConnectorOutcome,
        CeremonyExecutionConnectorPort, CeremonyExecutionRequest, ExecutionReceiptStorePort,
        PutArtifactChunk,
    };
    use made_core::value_objects::{
        ArtifactDigest, ArtifactProvenance, ArtifactSizeBytes, ArtifactSourceKind, AuditActorKind,
        CeremonyId, ExecutionConnectorId, ExecutionIntent, ExecutionOperation, ExecutionReceipt,
        ExecutionReceiptId, ExecutionRecoveryCapability, ExecutionRequestBytes, StateIteration,
        StateVisit, StepClaimFence, StepId, StepIteration, StepOutput, StepResult,
    };
    use sha2::{Digest, Sha256};

    #[derive(Debug)]
    struct NonQueryableConnector {
        id: ExecutionConnectorId,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl CeremonyExecutionConnectorPort for NonQueryableConnector {
        fn connector_id(&self) -> &ExecutionConnectorId {
            &self.id
        }

        fn recovery_capability(&self) -> ExecutionRecoveryCapability {
            ExecutionRecoveryCapability::ReconciliationRequired
        }

        fn source_kind(&self) -> ArtifactSourceKind {
            ArtifactSourceKind::ExternalExecution
        }

        async fn execute_or_recover(
            &self,
            _request: CeremonyExecutionRequest,
        ) -> Result<CeremonyExecutionConnectorOutcome, made_core::DomainError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(made_core::DomainError::InvariantViolated {
                reason: "non-queryable connector must not be replayed during recovery",
            })
        }
    }

    std::fs::create_dir_all("tmp").unwrap();
    let directory = TempDir::new_in("tmp").unwrap();
    let root = directory.path().canonicalize().unwrap();
    let policy_store =
        Arc::new(SqliteAuthorizationPolicyStore::open(root.join("source.sqlite3")).unwrap());
    let administration = AuthorizationPolicyAdministrationService::new(
        AuthorizationPolicyId::new("test-policy").unwrap(),
        policy_store,
        Arc::new(SystemClock::new()),
    );
    let owner = AuthenticatedPrincipal::new(
        PrincipalId::new("test-host").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    administration.open(owner.clone(), vec![]).await.unwrap();
    administration
        .issue(
            &owner,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("reconcile-non-queryable").unwrap(),
                owner.id().clone(),
                [AuthorizationAction::ReconcileExecutionOperation],
                AuthorizationScope::Global,
                (OffsetDateTime::UNIX_EPOCH, None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(owner.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();

    let store = Arc::new(SqliteCeremonyStore::open(root.join("source.sqlite3")).unwrap());
    let operation = ExecutionOperation::new(
        CeremonyId::new("non-queryable-crash").unwrap(),
        StepId::new("external").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
        ExecutionRequestBytes::new(b"possible external effect".to_vec()).unwrap(),
    );
    let fence = StepClaimFence::new("7".repeat(64)).unwrap();
    let connector_id = ExecutionConnectorId::new("manual").unwrap();
    let intent = ExecutionIntent::new(
        operation.clone(),
        fence.clone(),
        connector_id.clone(),
        ExecutionRecoveryCapability::ReconciliationRequired,
        ArtifactSourceKind::ExternalExecution,
        AuditActorKind::Engine,
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    store.record_intent(intent.clone()).await.unwrap();

    let artifacts = LocalArtifactStore::open(root.join("artifacts")).unwrap();
    let bytes = b"operator verified the non-queryable effect";
    let digest = ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(bytes))).unwrap();
    let upload = artifacts
        .begin_upload(BeginArtifactUpload {
            requested_artifact_id: None,
            expected_digest: digest.clone(),
            size_bytes: ArtifactSizeBytes::new(bytes.len() as u64),
            media_type: ArtifactMediaType::new("text/plain").unwrap(),
            provenance: ArtifactProvenance::execution(
                ArtifactSourceKind::ExternalExecution,
                ExecutionReceiptId::for_operation(operation.operation_id()),
                operation.operation_id().clone(),
                fence.clone(),
                OffsetDateTime::UNIX_EPOCH,
            )
            .unwrap(),
            idempotency_key: ArtifactIdempotencyKey::new("non-queryable-proof").unwrap(),
        })
        .await
        .unwrap();
    artifacts
        .put_chunk(PutArtifactChunk {
            upload_id: upload.upload_id.clone(),
            offset: ArtifactByteOffset::ZERO,
            chunk_digest: digest,
            bytes: bytes.to_vec(),
        })
        .await
        .unwrap();
    let evidence = artifacts.commit_upload(&upload.upload_id).await.unwrap();
    let receipt = ExecutionReceipt::new(
        operation.operation_id().clone(),
        operation.request_digest().clone(),
        fence.clone(),
        connector_id.clone(),
        None,
        ExecutionRecoveryCapability::ReconciliationRequired,
        ArtifactSourceKind::ExternalExecution,
        StepResult::completed(StepOutput::empty()).unwrap(),
        vec![evidence],
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    let request = json!({"command":{"operation":"reconcile_execution","receipt":receipt}});

    assert!(!run(&root, "--request", &request).status.success());
    assert!(store
        .reconciliation_requirement(operation.operation_id(), &fence)
        .await
        .unwrap()
        .is_none());
    let connector = Arc::new(NonQueryableConnector {
        id: connector_id,
        calls: AtomicUsize::new(0),
    });
    assert!(matches!(
        RecoverExecutionIntentUseCase::new(store.clone(), connector.clone())
            .execute(&intent)
            .await
            .unwrap(),
        RecoverExecutionIntentOutcome::ReconciliationRequired(_)
    ));
    assert_eq!(connector.calls.load(Ordering::SeqCst), 0);
    let reconciled = run(&root, "--request", &request);
    assert!(
        reconciled.status.success(),
        "{}",
        String::from_utf8_lossy(&reconciled.stderr)
    );
    assert_eq!(
        store.receipt(operation.operation_id()).await.unwrap(),
        Some(receipt)
    );
    assert_eq!(
        store.intents(operation.operation_id()).await.unwrap(),
        vec![intent]
    );
    assert!(store
        .reconciliation_requirement(operation.operation_id(), &fence)
        .await
        .unwrap()
        .is_some());
}
