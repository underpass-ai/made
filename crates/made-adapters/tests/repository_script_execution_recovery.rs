#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use made_adapters::execution::RepositoryScriptExecutionConnector;
use made_core::ports::{
    CeremonyExecutionConnectorOutcome, CeremonyExecutionConnectorPort, CeremonyExecutionRequest,
    CeremonyStepHandlerRequest,
};
use made_core::value_objects::{
    ArtifactSourceKind, Attributes, AuditActorKind, CeremonyContext, CeremonyId, CeremonyName,
    CeremonyVersion, ExecutionConnectorId, ExecutionIntent, ExecutionOperation,
    ExecutionRecoveryCapability, StateId, StateIteration, StateVisit, StepAttempt, StepClaimFence,
    StepHandlerConfig, StepHandlerKind, StepId, StepIteration,
};
use time::OffsetDateTime;

const NORMAL_SCRIPT: &str = r#"#!/usr/bin/env python3
import json, os, pathlib, sys, time
operation_id, request_digest, claim_fence, request_path, result_path = sys.argv[1:]
repository = pathlib.Path(__file__).parent
with (repository / "invocations.log").open("a", encoding="utf-8") as log:
    log.write(operation_id + "\n"); log.flush(); os.fsync(log.fileno())
time.sleep(0.1)
effect = repository / "materialized.txt"
temporary_effect = repository / ("." + operation_id + ".effect.tmp")
temporary_effect.write_bytes(pathlib.Path(request_path).read_bytes())
os.replace(temporary_effect, effect)
result = {"operation_id": operation_id, "request_digest": request_digest,
          "producer_claim_fence": claim_fence,
          "result": {"status": "COMPLETED", "output": {}},
          "observed_at": "1970-01-01T00:00:00Z"}
result_path = pathlib.Path(result_path)
temporary_result = result_path.parent / ("." + operation_id + ".result.tmp")
with temporary_result.open("w", encoding="utf-8") as output:
    json.dump(result, output, separators=(",", ":")); output.flush(); os.fsync(output.fileno())
os.replace(temporary_result, result_path)
directory = os.open(result_path.parent, os.O_RDONLY)
try: os.fsync(directory)
finally: os.close(directory)
"#;

const CRASH_AFTER_EFFECT_SCRIPT: &str = r#"#!/usr/bin/env python3
import os, pathlib, sys
operation_id, _, _, request_path, _ = sys.argv[1:]
repository = pathlib.Path(__file__).parent
with (repository / "invocations.log").open("a", encoding="utf-8") as log:
    log.write(operation_id + "\n"); log.flush(); os.fsync(log.fileno())
effect = repository / "materialized.txt"
effect.write_bytes(pathlib.Path(request_path).read_bytes())
with effect.open("rb") as stored: os.fsync(stored.fileno())
os._exit(17)
"#;

fn request() -> CeremonyExecutionRequest {
    let handler = CeremonyStepHandlerRequest::new(
        CeremonyId::new("script-recovery").unwrap(),
        CeremonyName::new("repository_script").unwrap(),
        CeremonyVersion::v1(),
        StateId::new("OPEN").unwrap(),
        StepId::new("materialize").unwrap(),
        StepHandlerKind::new("repository_script").unwrap(),
        StepHandlerConfig::new(Attributes::empty()),
        CeremonyContext::empty(),
        StepAttempt::FIRST,
    );
    let operation = ExecutionOperation::new(
        handler.instance_id().clone(),
        handler.step_id().clone(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
        handler.semantic_request_bytes().unwrap(),
    );
    let intent = ExecutionIntent::new(
        operation,
        StepClaimFence::new("a".repeat(64)).unwrap(),
        ExecutionConnectorId::new("local.repository-script.test").unwrap(),
        ExecutionRecoveryCapability::QueryableByOperationId,
        ArtifactSourceKind::ExternalExecution,
        AuditActorKind::Engine,
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    CeremonyExecutionRequest::new(intent, handler).unwrap()
}

fn write_script(repository: &Path, contents: &str) {
    let script_path = repository.join("worker.py");
    fs::write(&script_path, contents).unwrap();
    let mut permissions = fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(script_path, permissions).unwrap();
}

fn connector(repository: &Path, operation_root: &Path) -> RepositoryScriptExecutionConnector {
    RepositoryScriptExecutionConnector::new(
        ExecutionConnectorId::new("local.repository-script.test").unwrap(),
        repository,
        "worker.py",
        operation_root,
        Duration::from_secs(5),
    )
    .unwrap()
}

fn scratch() -> tempfile::TempDir {
    fs::create_dir_all("tmp").unwrap();
    tempfile::tempdir_in("tmp").unwrap()
}

#[tokio::test]
async fn restart_queries_a_durable_result_without_repeating_work() {
    let repository = scratch();
    let operation_root = scratch();
    write_script(repository.path(), NORMAL_SCRIPT);
    let request = request();
    let intent = request.intent().clone();
    let first = connector(repository.path(), operation_root.path());
    assert!(matches!(
        first.execute_or_recover(request).await.unwrap(),
        CeremonyExecutionConnectorOutcome::Observed(_)
    ));

    let reopened = connector(repository.path(), operation_root.path());
    assert!(matches!(
        reopened.recover_intent(&intent).await.unwrap(),
        CeremonyExecutionConnectorOutcome::Observed(_)
    ));
    assert_eq!(
        fs::read_to_string(repository.path().join("invocations.log"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[tokio::test]
async fn two_processes_share_one_durable_admission() {
    let repository = scratch();
    let operation_root = scratch();
    write_script(repository.path(), NORMAL_SCRIPT);
    let left = connector(repository.path(), operation_root.path());
    let right = connector(repository.path(), operation_root.path());
    let request = request();

    let (left, right) = tokio::join!(
        left.execute_or_recover(request.clone()),
        right.execute_or_recover(request)
    );

    assert!(left.is_ok());
    assert!(right.is_ok());
    assert_eq!(
        fs::read_to_string(repository.path().join("invocations.log"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[tokio::test]
async fn crash_after_effect_stays_ambiguous_and_is_never_reinvoked() {
    let repository = scratch();
    let operation_root = scratch();
    write_script(repository.path(), CRASH_AFTER_EFFECT_SCRIPT);
    let request = request();
    let intent = request.intent().clone();
    let first = connector(repository.path(), operation_root.path());
    assert!(matches!(
        first.execute_or_recover(request).await.unwrap(),
        CeremonyExecutionConnectorOutcome::ReconciliationRequired(_)
    ));

    let reopened = connector(repository.path(), operation_root.path());
    assert!(matches!(
        reopened.recover_intent(&intent).await.unwrap(),
        CeremonyExecutionConnectorOutcome::ReconciliationRequired(_)
    ));
    assert!(repository.path().join("materialized.txt").is_file());
    assert_eq!(
        fs::read_to_string(repository.path().join("invocations.log"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}
