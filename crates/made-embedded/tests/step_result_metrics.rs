use std::sync::Arc;

use made_adapters::metrics::PrometheusMetricsRecorder;
use made_app::usecases::{CompleteCeremonyStepInput, StartCeremonyInput, StartCeremonyStepInput};
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyId, DurationMs, IdempotencyKey, LeaseOwnerId, RoleId,
    StepErrorMessage, StepId, StepOutput, StepResult, StepStatus,
};
use made_embedded::EmbeddedMade;

const DEFINITION: &str = r#"
version: "1.0"
name: result_metrics
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
    handler: host_callback
roles:
  - id: WORKER
    allowed_actions: [work, finish]
"#;

#[tokio::test]
async fn every_observable_result_keeps_its_status_in_the_stream_metric() {
    let recorder = Arc::new(PrometheusMetricsRecorder::new().unwrap());
    let engine = EmbeddedMade::builder()
        .with_metrics(recorder.clone())
        .build();
    let statuses = [
        StepStatus::Completed,
        StepStatus::Failed,
        StepStatus::WaitingForHuman,
        StepStatus::Cancelled,
    ];
    for status in statuses {
        let error = (status == StepStatus::Failed)
            .then(|| StepErrorMessage::new("the provider failed").unwrap());
        complete(
            &engine,
            StepResult::new(status, StepOutput::empty(), error).unwrap(),
        )
        .await;
    }

    let rendered = recorder.render().unwrap();
    for status in statuses {
        let sample = format!(
            "made_ceremony_step_total{{ceremony=\"result_metrics\",status=\"{}\",step=\"work\"}} 1",
            status.as_label()
        );
        assert!(
            rendered.lines().any(|line| line == sample),
            "missing {sample}:\n{rendered}"
        );
    }
    assert_eq!(
        rendered
            .lines()
            .filter(|line| line.starts_with("made_ceremony_step_total{"))
            .count(),
        4,
        "one distinct status series for each sealed result"
    );
}

async fn complete(engine: &EmbeddedMade, result: StepResult) {
    let definition = engine.mount_yaml(DEFINITION).await.unwrap().definitions()[0].clone();
    let status = result.status();
    let id = CeremonyId::new(format!("result-{}", status.as_label())).unwrap();
    let step_id = StepId::new("work").unwrap();
    engine
        .start(StartCeremonyInput::new(
            id.clone(),
            definition.name().clone(),
            definition.version().clone(),
            CeremonyContext::empty(),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    let claim = engine
        .start_step(StartCeremonyStepInput::new(
            id.clone(),
            RoleId::new("WORKER").unwrap(),
            AuditActorKind::Agent,
            step_id.clone(),
            LeaseOwnerId::new("host").unwrap(),
            IdempotencyKey::new(format!("claim-{}", status.as_label())).unwrap(),
            DurationMs::from_millis(30_000),
        ))
        .await
        .unwrap();
    let instance = engine
        .complete_step(CompleteCeremonyStepInput::new(
            id,
            step_id.clone(),
            result,
            AuditActorKind::Agent,
            claim.claim_fence().clone(),
        ))
        .await
        .unwrap();
    assert_eq!(instance.step_record(&step_id).unwrap().status(), status);
}
