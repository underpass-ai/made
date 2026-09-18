use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use made_adapters::clock::SystemClock;
use made_adapters::memory::{
    ForgetfulMemory, InMemoryCeremonyDefinitionPublications, InMemoryCeremonyDefinitionRepository,
    InMemoryCeremonyEventStore,
};
use made_adapters::noop::NoopCeremonyStepHandler;
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::services::{SessionMemoryRecorder, SessionStream};
use made_app::usecases::{
    ResolveCeremonyDefinitionUseCase, RunCeremonyInput, RunCeremonyStepInput,
    RunCeremonyStepUseCase, RunCeremonyUseCase, StartCeremonyInput, StartCeremonyUseCase,
};
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyDefinitionRepositoryPort, CeremonyStepHandlerPort, CeremonyStepHandlerRequest,
};
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyContext, CeremonyId, DurationMs, IdempotencyKey,
    LeaseOwnerId, RoleId, StepId, StepOutput, StepResult,
};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::Value;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SimpleSpanProcessor};
use tracing_subscriber::layer::SubscriberExt as _;

const LINEAR_CEREMONY: &str = r#"
version: "1.0"
name: span_linear
states:
  - id: STARTED
    initial: true
  - id: COMPLETED
    terminal: true
transitions:
  - from: STARTED
    to: COMPLETED
    trigger: finish
    guards: [work_completed]
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
    allowed_actions: [work, finish]
"#;

const REPEATING_STATE_CEREMONY: &str = r#"
version: "1.0"
name: span_repeating_state
states:
  - id: STARTED
    initial: true
    repeat:
      max_iterations: 2
      until:
        step: work
        output_field: ready
        equals: true
  - id: COMPLETED
    terminal: true
transitions:
  - from: STARTED
    to: COMPLETED
    trigger: finish
    guards: [work_completed]
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
    allowed_actions: [work, finish]
"#;

fn install_bridge() -> (InMemorySpanExporter, tracing::subscriber::DefaultGuard) {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
        .build();
    let tracer = provider.tracer("ceremony-step-span-tests");
    let subscriber = tracing_subscriber::registry().with(
        tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_context_activation(false),
    );
    (exporter, tracing::subscriber::set_default(subscriber))
}

fn stream() -> Arc<SessionStream> {
    let store = Arc::new(InMemoryCeremonyEventStore::new());
    Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(SessionMemoryRecorder::new(
            Arc::new(ForgetfulMemory::new()),
            store,
        )),
    ))
}

fn input(id: &str) -> RunCeremonyInput {
    RunCeremonyInput::new(
        CeremonyId::new(id).unwrap(),
        CeremonyDefinitionYaml::parse_str(LINEAR_CEREMONY).unwrap(),
        CeremonyContext::empty(),
        LeaseOwnerId::new("span-host").unwrap(),
        DurationMs::from_millis(30_000),
        "operator",
        AuditActorKind::Service,
    )
}

fn string_attr(span: &opentelemetry_sdk::trace::SpanData, key: &str) -> Option<String> {
    span.attributes
        .iter()
        .find(|attribute| attribute.key.as_str() == key)
        .and_then(|attribute| match &attribute.value {
            Value::String(value) => Some(value.to_string()),
            _ => None,
        })
}

fn int_attr(span: &opentelemetry_sdk::trace::SpanData, key: &str) -> Option<i64> {
    span.attributes
        .iter()
        .find(|attribute| attribute.key.as_str() == key)
        .and_then(|attribute| match attribute.value {
            Value::I64(value) => Some(value),
            _ => None,
        })
}

fn span_for(
    spans: &[opentelemetry_sdk::trace::SpanData],
    name: &str,
    ceremony_id: &str,
) -> opentelemetry_sdk::trace::SpanData {
    spans
        .iter()
        .find(|span| {
            span.name == name && string_attr(span, "ceremony_id").as_deref() == Some(ceremony_id)
        })
        .unwrap_or_else(|| panic!("no {name} span for {ceremony_id}"))
        .clone()
}

fn state_iterations_for(
    spans: &[opentelemetry_sdk::trace::SpanData],
    name: &str,
    ceremony_id: &str,
) -> Vec<i64> {
    spans
        .iter()
        .filter(|span| {
            span.name == name && string_attr(span, "ceremony_id").as_deref() == Some(ceremony_id)
        })
        .map(|span| {
            int_attr(span, "state_iteration")
                .unwrap_or_else(|| panic!("{name} span has no state_iteration: {span:?}"))
        })
        .collect()
}

#[derive(Debug)]
struct FailingHandler;

#[async_trait]
impl CeremonyStepHandlerPort for FailingHandler {
    async fn execute(
        &self,
        _request: CeremonyStepHandlerRequest,
    ) -> Result<made_core::value_objects::StepResult, DomainError> {
        Err(DomainError::InvariantViolated {
            reason: "handler refused test work",
        })
    }
}

#[derive(Debug, Default)]
struct ReadinessSequenceHandler {
    calls: AtomicUsize,
}

#[async_trait]
impl CeremonyStepHandlerPort for ReadinessSequenceHandler {
    async fn execute(
        &self,
        _request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        let ready = self.calls.fetch_add(1, Ordering::SeqCst) > 0;
        StepResult::completed(StepOutput::new(
            Attributes::new(BTreeMap::from([("ready".to_owned(), ready.into())])).unwrap(),
        ))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn one_shot_exports_step_and_handler_topology_with_late_fields() {
    let (exporter, _guard) = install_bridge();
    let usecase = RunCeremonyUseCase::new(
        Arc::new(InMemoryCeremonyDefinitionRepository::new()),
        stream(),
        Arc::new(NoopCeremonyStepHandler::new()),
        Arc::new(SystemClock::new()),
    );

    usecase.execute(input("span-one-shot")).await.unwrap();

    let spans = exporter.get_finished_spans().unwrap();
    let run = span_for(&spans, "run_ceremony", "span-one-shot");
    let step = span_for(&spans, "ceremony_step", "span-one-shot");
    let handler = span_for(&spans, "ceremony_step_handler", "span-one-shot");
    assert_eq!(step.parent_span_id, run.span_context.span_id());
    assert_eq!(handler.parent_span_id, step.span_context.span_id());
    assert_eq!(
        string_attr(&step, "ceremony_name").as_deref(),
        Some("span_linear")
    );
    assert_eq!(string_attr(&step, "state_id").as_deref(), Some("STARTED"));
    assert_eq!(string_attr(&step, "step_id").as_deref(), Some("work"));
    assert_eq!(string_attr(&step, "role_id").as_deref(), Some("SYSTEM"));
    assert_eq!(
        int_attr(&step, "iteration"),
        Some(1),
        "{:?}",
        step.attributes
    );
    assert_eq!(int_attr(&step, "attempt"), Some(1));
    assert_eq!(string_attr(&step, "outcome").as_deref(), Some("success"));
    assert_eq!(
        string_attr(&step, "step_status").as_deref(),
        Some("completed")
    );
    assert_eq!(
        string_attr(&handler, "handler_kind").as_deref(),
        Some("host_callback")
    );
    assert_eq!(int_attr(&handler, "attempt"), Some(1));
    assert_eq!(string_attr(&handler, "outcome").as_deref(), Some("success"));
}

#[tokio::test(flavor = "current_thread")]
async fn state_repeat_spans_keep_the_state_iteration_that_each_step_executed_in() {
    let (exporter, _guard) = install_bridge();
    let definition = CeremonyDefinitionYaml::parse_str(REPEATING_STATE_CEREMONY).unwrap();
    let one_shot = RunCeremonyUseCase::new(
        Arc::new(InMemoryCeremonyDefinitionRepository::new()),
        stream(),
        Arc::new(ReadinessSequenceHandler::default()),
        Arc::new(SystemClock::new()),
    );

    one_shot
        .execute(RunCeremonyInput::new(
            CeremonyId::new("span-state-repeat").unwrap(),
            definition.clone(),
            CeremonyContext::empty(),
            LeaseOwnerId::new("span-host").unwrap(),
            DurationMs::from_millis(30_000),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();

    let definitions = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    let stream = stream();
    let clock = Arc::new(SystemClock::new());
    definitions.save(&definition).await.unwrap();
    StartCeremonyUseCase::new(
        definitions.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    )
    .execute(StartCeremonyInput::new(
        CeremonyId::new("span-delegated-repeat").unwrap(),
        definition.name().clone(),
        definition.version().clone(),
        CeremonyContext::empty(),
        "operator",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
    let delegated = RunCeremonyStepUseCase::new(
        Arc::new(ResolveCeremonyDefinitionUseCase::new(
            definitions,
            Arc::new(InMemoryCeremonyDefinitionPublications::new()),
        )),
        stream,
        Arc::new(ReadinessSequenceHandler::default()),
        clock,
    );

    let first = delegated
        .execute(repeating_step_input("span-delegated-repeat", 1))
        .await
        .unwrap();
    assert_eq!(first.instance().current_state_iteration().get(), 2);
    delegated
        .execute(repeating_step_input("span-delegated-repeat", 2))
        .await
        .unwrap();

    let spans = exporter.get_finished_spans().unwrap();
    assert_eq!(
        state_iterations_for(&spans, "ceremony_step", "span-state-repeat"),
        [1, 2]
    );
    assert_eq!(
        state_iterations_for(&spans, "run_ceremony_step", "span-delegated-repeat"),
        [1, 2],
        "the first delegated span must retain the claimed state iteration even though completion opens the next one"
    );
}

fn repeating_step_input(ceremony_id: &str, invocation: usize) -> RunCeremonyStepInput {
    RunCeremonyStepInput::new(
        CeremonyId::new(ceremony_id).unwrap(),
        RoleId::new("SYSTEM").unwrap(),
        AuditActorKind::Service,
        StepId::new("work").unwrap(),
        LeaseOwnerId::new("span-host").unwrap(),
        IdempotencyKey::new(format!("{ceremony_id}:work:{invocation}")).unwrap(),
        DurationMs::from_millis(30_000),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn normalized_handler_failure_and_explicit_step_parent_are_exported() {
    let (exporter, _guard) = install_bridge();
    let failed_one_shot = RunCeremonyUseCase::new(
        Arc::new(InMemoryCeremonyDefinitionRepository::new()),
        stream(),
        Arc::new(FailingHandler),
        Arc::new(SystemClock::new()),
    );
    assert!(failed_one_shot
        .execute(input("span-one-shot-failed"))
        .await
        .is_err());

    let definitions = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    let stream = stream();
    let clock = Arc::new(SystemClock::new());
    let definition = CeremonyDefinitionYaml::parse_str(LINEAR_CEREMONY).unwrap();
    definitions.save(&definition).await.unwrap();
    StartCeremonyUseCase::new(
        definitions.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    )
    .execute(StartCeremonyInput::new(
        CeremonyId::new("span-explicit").unwrap(),
        definition.name().clone(),
        definition.version().clone(),
        CeremonyContext::empty(),
        "operator",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        definitions,
        Arc::new(InMemoryCeremonyDefinitionPublications::new()),
    ));
    let output = RunCeremonyStepUseCase::new(resolver, stream, Arc::new(FailingHandler), clock)
        .execute(RunCeremonyStepInput::new(
            CeremonyId::new("span-explicit").unwrap(),
            RoleId::new("SYSTEM").unwrap(),
            AuditActorKind::Service,
            StepId::new("work").unwrap(),
            LeaseOwnerId::new("span-host").unwrap(),
            IdempotencyKey::new("span-explicit:work:1").unwrap(),
            DurationMs::from_millis(30_000),
        ))
        .await
        .unwrap();
    assert!(!output.result().is_success());

    let spans = exporter.get_finished_spans().unwrap();
    let failed_step = span_for(&spans, "ceremony_step", "span-one-shot-failed");
    let failed_handler = span_for(&spans, "ceremony_step_handler", "span-one-shot-failed");
    assert_eq!(
        string_attr(&failed_step, "error_kind").as_deref(),
        Some("step_result_failed")
    );
    assert_eq!(
        string_attr(&failed_handler, "error_kind").as_deref(),
        Some("invariant_violated")
    );
    assert_eq!(
        string_attr(&failed_handler, "step_status").as_deref(),
        Some("failed")
    );

    let run = span_for(&spans, "run_ceremony_step", "span-explicit");
    let handler = span_for(&spans, "ceremony_step_handler", "span-explicit");
    assert_eq!(handler.parent_span_id, run.span_context.span_id());
    assert_eq!(string_attr(&handler, "outcome").as_deref(), Some("error"));
    assert_eq!(
        string_attr(&handler, "error_kind").as_deref(),
        Some("invariant_violated")
    );
    assert_eq!(
        string_attr(&handler, "step_status").as_deref(),
        Some("failed")
    );
}
