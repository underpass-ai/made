//! What the in-process engine says about itself.
//!
//! `status` and `metrics` are the same two use cases the deployable
//! edition serves `GetStatus` and `GetMetrics` with (ADR-014), so what
//! is worth asserting here is the part that is the embedded edition's
//! own: which recorder a host gets when it wires none, that the
//! recorder really receives what a session does, and that the counters
//! an edition without a council keeps are honest zeros rather than an
//! absent answer.

use std::sync::Arc;

use made_adapters::metrics::PrometheusMetricsRecorder;
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::usecases::{
    ApplyCeremonyTransitionInput, RunCeremonyInput, RunCeremonyStepInput, ServiceHealth,
    StartCeremonyInput,
};
use made_core::ports::NoopMetricsRecorder;
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyId, DurationMs, IdempotencyKey, LeaseOwnerId, RoleId,
    StepId, TransitionTrigger,
};
use made_embedded::{EmbeddedMade, VERSION};

const LINEAR_CEREMONY: &str = r#"
version: "1.0"
name: "observability_linear"
states:
  - id: STARTED
    initial: true
  - id: COMPLETED
    terminal: true
transitions:
  - from: STARTED
    to: COMPLETED
    trigger: finish
    guards:
      - work_completed
steps:
  - id: work
    state: STARTED
    handler: host_callback
guards:
  work_completed:
    type: automated
    check: "step_status:work:COMPLETED"
roles:
  - id: SYSTEM
    allowed_actions:
      - work
      - finish
"#;

fn one_run(ceremony_id: &str) -> RunCeremonyInput {
    RunCeremonyInput::new(
        CeremonyId::new(ceremony_id).unwrap(),
        CeremonyDefinitionYaml::parse_str(LINEAR_CEREMONY).unwrap(),
        CeremonyContext::empty(),
        LeaseOwnerId::new("observability-host").unwrap(),
        DurationMs::from_millis(30_000),
        "operator-1",
        AuditActorKind::Service,
    )
}

/// A host that configures nothing still gets a registry. The status
/// says which one, because "the recorder is the host's choice" and
/// "nothing is recording" look identical from outside otherwise.
#[tokio::test]
async fn an_engine_that_was_handed_no_recorder_still_has_one_and_names_it() {
    let status = EmbeddedMade::default().status(false).await.unwrap();

    assert_eq!(status.version(), VERSION);
    assert_eq!(status.health(), ServiceHealth::Healthy);
    assert_eq!(status.recorder(), "prometheus");
    assert!(status.statistics().is_none());
}

#[tokio::test]
async fn a_host_that_wires_its_own_recorder_is_what_the_status_names() {
    let status = EmbeddedMade::builder()
        .with_metrics(Arc::new(NoopMetricsRecorder))
        .build()
        .status(false)
        .await
        .unwrap();

    assert_eq!(status.recorder(), "noop");
}

/// The default registry is not decoration: a session that runs lands
/// in it. Injected here rather than read back out of the engine only
/// because rendering a registry is a concrete adapter's job — the
/// engine wires this very type when a host wires none.
#[tokio::test]
async fn a_session_that_runs_moves_the_ceremony_families() {
    let recorder = Arc::new(PrometheusMetricsRecorder::new().unwrap());
    let engine = EmbeddedMade::builder()
        .with_observability(recorder.clone())
        .build();

    assert!(
        !recorder
            .render()
            .unwrap()
            .contains("made_ceremony_completed_total{"),
        "nothing has run yet, so no ceremony family has a sample"
    );

    let output = Box::pin(engine.run(one_run("observability-run")))
        .await
        .unwrap();
    assert!(output.instance().is_completed(output.definition()));

    let rendered = recorder.render().unwrap();
    assert!(
        rendered.contains(
            "made_ceremony_completed_total{ceremony=\"observability_linear\",outcome=\"completed\"} 1"
        ),
        "the run did not reach the recorder the host wired:\n{rendered}"
    );
    assert!(
        rendered.contains(
            "made_ceremony_step_total{ceremony=\"observability_linear\",status=\"completed\",step=\"work\"} 1"
        ),
        "the step did not reach the recorder the host wired:\n{rendered}"
    );
    assert_eq!(engine.status(false).await.unwrap().recorder(), "prometheus");
    let observed = engine.metrics().await.unwrap();
    assert_eq!(observed.registry().text().as_str(), rendered);
    assert!(observed
        .registry()
        .families()
        .iter()
        .any(|family| family.name().as_str() == "made_ceremony_step_total"));
}

#[tokio::test]
async fn one_shot_and_step_drivers_emit_the_same_ceremony_metric_deltas() {
    let one_shot_metrics = Arc::new(PrometheusMetricsRecorder::new().unwrap());
    let one_shot = EmbeddedMade::builder()
        .with_metrics(one_shot_metrics.clone())
        .build();
    Box::pin(one_shot.run(one_run("one-shot-metrics")))
        .await
        .unwrap();

    let step_metrics = Arc::new(PrometheusMetricsRecorder::new().unwrap());
    let step = EmbeddedMade::builder()
        .with_metrics(step_metrics.clone())
        .build();
    let definition = step
        .mount_yaml(LINEAR_CEREMONY)
        .await
        .unwrap()
        .definitions()[0]
        .clone();
    let ceremony_id = CeremonyId::new("step-metrics").unwrap();
    step.start(StartCeremonyInput::new(
        ceremony_id.clone(),
        definition.name().clone(),
        definition.version().clone(),
        CeremonyContext::empty(),
        "operator-1",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
    Box::pin(step.run_step(RunCeremonyStepInput::new(
        ceremony_id.clone(),
        RoleId::new("SYSTEM").unwrap(),
        AuditActorKind::Service,
        StepId::new("work").unwrap(),
        LeaseOwnerId::new("observability-host").unwrap(),
        IdempotencyKey::new("step-metrics:work:1").unwrap(),
        DurationMs::from_millis(30_000),
    )))
    .await
    .unwrap();
    step.apply_transition(ApplyCeremonyTransitionInput::new(
        ceremony_id,
        RoleId::new("SYSTEM").unwrap(),
        AuditActorKind::Service,
        TransitionTrigger::new("finish").unwrap(),
    ))
    .await
    .unwrap();

    assert_eq!(
        comparable_ceremony_metrics(&one_shot_metrics.render().unwrap()),
        comparable_ceremony_metrics(&step_metrics.render().unwrap())
    );
}

fn comparable_ceremony_metrics(rendered: &str) -> Vec<&str> {
    rendered
        .lines()
        .filter(|line| line.starts_with("made_ceremony_"))
        .filter(|line| !line.contains("_bucket{") && !line.contains("_sum{"))
        .collect()
}

/// Every family the deployable edition reports, present and zero.
///
/// `Statistics` counts deliberations and orchestrations — council
/// work — and the embedded edition runs no council, so these stay at
/// zero however many sessions it runs. Zero is the true count; an
/// absent `stats` would have been a different claim, and the wrong
/// one. The registry above is where a ceremony shows up, and G3 is
/// what puts it on this answer.
#[tokio::test]
async fn the_counters_answer_with_every_family_at_zero_before_and_after_a_session() {
    let engine = EmbeddedMade::default();

    let before = engine.metrics().await.unwrap();
    assert_eq!(before.statistics().total_deliberations(), 0);
    assert_eq!(before.statistics().total_orchestrations(), 0);
    assert_eq!(before.statistics().total_duration(), DurationMs::ZERO);
    assert_eq!(before.statistics().average_duration_ms(), 0.0);
    assert!(before.statistics().per_specialty().is_empty());

    Box::pin(engine.run(one_run("observability-counters")))
        .await
        .unwrap();

    let after = engine.metrics().await.unwrap();
    assert_eq!(after.statistics(), before.statistics());
    assert!(after
        .registry()
        .text()
        .as_str()
        .contains("made_ceremony_completed_total"));
    assert_eq!(
        engine.status(true).await.unwrap().statistics(),
        Some(before.statistics()),
        "asking for the counters inside a status must answer with the same snapshot"
    );
}

/// Uptime is this engine's, not the host process's, and it is whole
/// seconds because that is what the contract carries.
#[tokio::test]
async fn a_freshly_built_engine_has_been_up_no_whole_seconds() {
    assert_eq!(
        EmbeddedMade::default()
            .status(false)
            .await
            .unwrap()
            .uptime_seconds(),
        0
    );
}
