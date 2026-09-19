//! Host-seeded deterministic receipts are consumed through every public backend.
use std::sync::Arc;

use made_core::ports::{
    ArtifactIdempotencyKey, ArtifactStorePort, BeginArtifactUpload, ExecutionReceiptStorePort,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactId, ArtifactMediaType, ArtifactProvenance, ArtifactSizeBytes,
    ArtifactSourceKind, AuditActorKind, CeremonyId, ExecutionConnectorId, ExecutionIntent,
    ExecutionOperation, ExecutionReceipt, ExecutionReceiptId, ExecutionRecoveryCapability,
    ExecutionRequestBytes, StateIteration, StateVisit, StepClaimFence, StepId, StepIteration,
    StepOutput, StepResult,
};
use made_tests_integration::parity_clock::PARITY_INSTANT;
use serde_json::{json, Value};

const DIRECT: &str = "parity-receipt-direct";
const ADOPTED: &str = "parity-receipt-adopted";
const YAML: &str = r#"version: "1.0"
name: parity_receipts
states:
  - {id: WORK, initial: true}
steps:
  - {id: work, state: WORK, handler: noop}
roles:
  - {id: DRIVER, allowed_actions: [work]}
"#;

fn operation(ceremony: &str) -> ExecutionOperation {
    ExecutionOperation::new(
        CeremonyId::new(ceremony).unwrap(),
        StepId::new("work").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
        ExecutionRequestBytes::new(b"explicit deterministic parity fixture".to_vec()).unwrap(),
    )
}

pub(super) async fn seed_after_claim(
    stores: &[Arc<dyn ExecutionReceiptStorePort>],
    artifacts: &[Arc<dyn ArtifactStorePort>],
    request: &Value,
    accepted: &str,
) {
    let Some(ceremony @ (DIRECT | ADOPTED)) = request["ceremony_id"].as_str() else {
        return;
    };
    let operation = operation(ceremony);
    // Adoption starts with a stored producer receipt whose fence differs from
    // the live claim. The dedicated process-recovery tests prove its history;
    // this fixture compares public mapping and fenced application, not an effect.
    let producer = if ceremony == ADOPTED {
        "e".repeat(64)
    } else {
        accepted.to_owned()
    };
    let producer = StepClaimFence::new(producer).unwrap();
    let connector = ExecutionConnectorId::new("parity.fixture").unwrap();
    let capability = ExecutionRecoveryCapability::QueryableByOperationId;
    let intent = ExecutionIntent::new(
        operation.clone(),
        producer.clone(),
        connector.clone(),
        capability,
        ArtifactSourceKind::Fixture,
        AuditActorKind::Agent,
        PARITY_INSTANT,
    )
    .unwrap();
    for (store, artifacts) in stores.iter().zip(artifacts) {
        let upload = artifacts
            .begin_upload(BeginArtifactUpload {
                requested_artifact_id: Some(
                    ArtifactId::new(format!("{ceremony}-artifact")).unwrap(),
                ),
                expected_digest: ArtifactDigest::new(
                    "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                )
                .unwrap(),
                size_bytes: ArtifactSizeBytes::new(0),
                media_type: ArtifactMediaType::new("application/octet-stream").unwrap(),
                provenance: ArtifactProvenance::execution(
                    ArtifactSourceKind::Fixture,
                    ExecutionReceiptId::for_operation(operation.operation_id()),
                    operation.operation_id().clone(),
                    producer.clone(),
                    PARITY_INSTANT,
                )
                .unwrap(),
                idempotency_key: ArtifactIdempotencyKey::new(ceremony).unwrap(),
            })
            .await
            .unwrap();
        let artifact = artifacts.commit_upload(&upload.upload_id).await.unwrap();
        let receipt = ExecutionReceipt::new(
            operation.operation_id().clone(),
            operation.request_digest().clone(),
            producer.clone(),
            connector.clone(),
            None,
            capability,
            ArtifactSourceKind::Fixture,
            StepResult::completed(StepOutput::empty()).unwrap(),
            vec![artifact],
            PARITY_INSTANT,
        )
        .unwrap();
        store.record_intent(intent.clone()).await.unwrap();
        store.record_receipt(receipt.clone()).await.unwrap();
    }
}

pub(super) fn script() -> Vec<(&'static str, Value)> {
    let mut calls = Vec::new();
    for (ceremony, completion) in [
        (DIRECT, "made_complete_execution_receipt"),
        (ADOPTED, "made_adopt_execution_receipt"),
    ] {
        let operation = operation(ceremony);
        calls.extend([
            ("made_start_ceremony", json!({"ceremony_id":ceremony, "definition_yaml":YAML, "actor_id":"parity-host", "actor_kind":"service"})),
            ("made_claim_ceremony_step", json!({"ceremony_id":ceremony, "step_id":"work", "actor_kind":"agent", "lease_owner_id":"parity-host", "idempotency_key":ceremony, "lease_ttl_ms":30_000})),
            ("made_get_execution_receipt", json!({"operation_id":operation.operation_id().as_str()})),
            ("made_inspect_execution_recovery", json!({"limit":1})),
            (completion, json!({"ceremony_id":ceremony, "step_id":"work", "actor_kind":"agent", "operation_id":operation.operation_id().as_str()})),
            ("made_get_ceremony_instance", json!({"ceremony_id":ceremony})),
            ("made_read_ceremony_events", json!({"ceremony_id":ceremony})),
            ("made_verify_ceremony_journal", json!({"ceremony_id":ceremony})),
        ]);
    }
    calls
}

pub(super) fn assert_result(tool: &str, arguments: &Value, result: &Value) {
    if tool == "made_get_execution_receipt" {
        assert_eq!(result["source_kind"], "fixture");
        assert_eq!(result["artifacts"].as_array().unwrap().len(), 1);
    }
    let Some(ceremony @ (DIRECT | ADOPTED)) = arguments["ceremony_id"].as_str() else {
        return;
    };
    if tool == "made_get_ceremony_instance" {
        assert_eq!(result["steps"][0]["status"], "completed");
    }
    if tool == "made_read_ceremony_events" {
        let links = result["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|record| record["event_type"] == "execution_receipt_linked")
            .collect::<Vec<_>>();
        assert_eq!(links.len(), 1);
        let link = &links[0]["event"]["link"];
        assert_eq!(
            link["operation_id"],
            operation(ceremony).operation_id().as_str()
        );
        assert_eq!(
            link["kind"],
            if ceremony == DIRECT {
                "direct"
            } else {
                "adopted"
            }
        );
        assert_eq!(
            link["producer_claim_fence"] == link["applied_claim_fence"],
            ceremony == DIRECT
        );
    }
}
