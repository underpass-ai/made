use made_core::ports::{CeremonyExecutionRequest, CeremonyStepHandlerRequest};
use made_core::value_objects::{
    ArtifactSourceKind, AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion,
    ExecutionConnectorId, ExecutionIntent, ExecutionOperation, ExecutionRecoveryCapability,
    StateId, StateIteration, StateVisit, StepAttempt, StepClaimFence, StepHandlerConfig,
    StepHandlerKind, StepId, StepIteration,
};

pub fn request(connector: &str, config: serde_json::Value) -> CeremonyExecutionRequest {
    let handler = CeremonyStepHandlerRequest::new(
        CeremonyId::new(format!("execution-{}", uuid::Uuid::new_v4())).unwrap(),
        CeremonyName::new("connector_execution").unwrap(),
        CeremonyVersion::v1(),
        StateId::new("OPEN").unwrap(),
        StepId::new("execute").unwrap(),
        StepHandlerKind::new("external").unwrap(),
        StepHandlerConfig::new(serde_json::from_value(config).unwrap()),
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
        ExecutionConnectorId::new(connector).unwrap(),
        ExecutionRecoveryCapability::QueryableByOperationId,
        ArtifactSourceKind::ExternalExecution,
        AuditActorKind::Engine,
        time::OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap();
    CeremonyExecutionRequest::new(intent, handler).unwrap()
}
pub fn scratch() -> tempfile::TempDir {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/connector-execution");
    std::fs::create_dir_all(&path).unwrap();
    tempfile::tempdir_in(path).unwrap()
}
