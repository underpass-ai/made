use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use made_adapters::clock::SystemClock;
use made_adapters::memory::{
    ForgetfulMemory, InMemoryProtocolDefinitionPublications, InMemoryProtocolDefinitionRepository,
    InMemoryProtocolEventStore,
};
use made_adapters::noop::NoopProtocolStepHandler;
use made_adapters::yaml::ProtocolDefinitionYaml;
use made_app::services::{SessionMemoryRecorder, SessionStream};
use made_app::usecases::{
    ResolveProtocolDefinitionUseCase, RunProtocolInput, RunProtocolStepInput,
    RunProtocolStepUseCase, RunProtocolUseCase, StartProtocolInput, StartProtocolUseCase,
};
use made_core::error::DomainError;
use made_core::ports::{
    ProtocolDefinitionRepositoryPort, ProtocolStepHandlerPort, ProtocolStepHandlerRequest,
};
use made_core::value_objects::{
    Attributes, AuditActorKind, DurationMs, IdempotencyKey, LeaseOwnerId, ProtocolContext,
    ProtocolId, RoleId, StepId, StepOutput, StepResult,
};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::Value;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SimpleSpanProcessor};
use tracing_subscriber::layer::SubscriberExt as _;

const LINEAR_PROTOCOL: &str = r#"
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

const REPEATING_STATE_PROTOCOL: &str = r#"
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

const CONCURRENT_PROTOCOL: &str = r#"
version: "1.0"
name: span_concurrent
states:
  - id: REVIEW
    initial: true
    execution: concurrent
  - id: COMPLETED
    terminal: true
transitions:
  - from: REVIEW
    to: COMPLETED
    trigger: finish
    guards: [both_completed]
steps:
  - {id: api, state: REVIEW, handler: host_callback}
  - {id: data, state: REVIEW, handler: host_callback}
guards:
  both_completed: {type: automated, check: "steps_completed:2"}
roles:
  - {id: API, allowed_actions: [api, finish]}
  - {id: DATA, allowed_actions: [data, finish]}
max_parallel: 2
"#;

fn install_bridge() -> (InMemorySpanExporter, tracing::subscriber::DefaultGuard) {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
        .build();
    let tracer = provider.tracer("protocol-step-span-tests");
    let subscriber = tracing_subscriber::registry().with(
        tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_context_activation(false),
    );
    (exporter, tracing::subscriber::set_default(subscriber))
}

fn stream() -> Arc<SessionStream> {
    let store = Arc::new(InMemoryProtocolEventStore::new());
    Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(SessionMemoryRecorder::new(
            Arc::new(ForgetfulMemory::new()),
            store,
        )),
    ))
}

fn input(id: &str) -> RunProtocolInput {
    RunProtocolInput::new(
        ProtocolId::new(id).unwrap(),
        ProtocolDefinitionYaml::parse_str(LINEAR_PROTOCOL).unwrap(),
        ProtocolContext::empty(),
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
    protocol_id: &str,
) -> opentelemetry_sdk::trace::SpanData {
    spans
        .iter()
        .find(|span| {
            span.name == name && string_attr(span, "protocol_id").as_deref() == Some(protocol_id)
        })
        .unwrap_or_else(|| panic!("no {name} span for {protocol_id}"))
        .clone()
}

fn state_iterations_for(
    spans: &[opentelemetry_sdk::trace::SpanData],
    name: &str,
    protocol_id: &str,
) -> Vec<i64> {
    spans
        .iter()
        .filter(|span| {
            span.name == name && string_attr(span, "protocol_id").as_deref() == Some(protocol_id)
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
impl ProtocolStepHandlerPort for FailingHandler {
    async fn execute(
        &self,
        _request: ProtocolStepHandlerRequest,
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
impl ProtocolStepHandlerPort for ReadinessSequenceHandler {
    async fn execute(
        &self,
        _request: ProtocolStepHandlerRequest,
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
    let usecase = RunProtocolUseCase::new(
        Arc::new(InMemoryProtocolDefinitionRepository::new()),
        stream(),
        Arc::new(NoopProtocolStepHandler::new()),
        Arc::new(SystemClock::new()),
    );

    usecase.execute(input("span-one-shot")).await.unwrap();

    let spans = exporter.get_finished_spans().unwrap();
    let run = span_for(&spans, "run_protocol", "span-one-shot");
    let step = span_for(&spans, "protocol_step", "span-one-shot");
    let handler = span_for(&spans, "protocol_step_handler", "span-one-shot");
    assert_eq!(step.parent_span_id, run.span_context.span_id());
    assert_eq!(handler.parent_span_id, step.span_context.span_id());
    assert_eq!(
        string_attr(&step, "protocol_name").as_deref(),
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
    assert_eq!(int_attr(&step, "state_visit"), Some(1));
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
async fn concurrent_steps_export_distinct_children_through_completion() {
    let (exporter, _guard) = install_bridge();
    let usecase = RunProtocolUseCase::new(
        Arc::new(InMemoryProtocolDefinitionRepository::new()),
        stream(),
        Arc::new(NoopProtocolStepHandler::new()),
        Arc::new(SystemClock::new()),
    );
    let protocol_id = "span-concurrent";
    usecase
        .execute(RunProtocolInput::new(
            ProtocolId::new(protocol_id).unwrap(),
            ProtocolDefinitionYaml::parse_str(CONCURRENT_PROTOCOL).unwrap(),
            ProtocolContext::empty(),
            LeaseOwnerId::new("span-host").unwrap(),
            DurationMs::from_millis(30_000),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();

    let spans = exporter.get_finished_spans().unwrap();
    let run = span_for(&spans, "run_protocol", protocol_id);
    let steps = spans
        .iter()
        .filter(|span| {
            span.name == "protocol_step"
                && string_attr(span, "protocol_id").as_deref() == Some(protocol_id)
        })
        .collect::<Vec<_>>();
    assert_eq!(steps.len(), 2, "{spans:#?}");
    for step in steps {
        let step_id = string_attr(step, "step_id").expect("step id is sealed on the span");
        assert_eq!(step.parent_span_id, run.span_context.span_id());
        assert_eq!(int_attr(step, "state_visit"), Some(1));
        assert_eq!(int_attr(step, "state_iteration"), Some(1));
        assert_eq!(int_attr(step, "iteration"), Some(1));
        assert_eq!(int_attr(step, "attempt"), Some(1));
        assert_eq!(string_attr(step, "outcome").as_deref(), Some("success"));
        assert_eq!(
            string_attr(step, "role_id").as_deref(),
            Some(if step_id == "api" { "API" } else { "DATA" })
        );
        let handlers = spans
            .iter()
            .filter(|handler| {
                handler.name == "protocol_step_handler"
                    && string_attr(handler, "protocol_id").as_deref() == Some(protocol_id)
                    && string_attr(handler, "step_id").as_deref() == Some(step_id.as_str())
            })
            .collect::<Vec<_>>();
        assert_eq!(handlers.len(), 1, "{spans:#?}");
        assert_eq!(handlers[0].parent_span_id, step.span_context.span_id());
    }
}

#[tokio::test(flavor = "current_thread")]
async fn state_repeat_spans_keep_the_state_iteration_that_each_step_executed_in() {
    let (exporter, _guard) = install_bridge();
    let definition = ProtocolDefinitionYaml::parse_str(REPEATING_STATE_PROTOCOL).unwrap();
    let one_shot = RunProtocolUseCase::new(
        Arc::new(InMemoryProtocolDefinitionRepository::new()),
        stream(),
        Arc::new(ReadinessSequenceHandler::default()),
        Arc::new(SystemClock::new()),
    );

    one_shot
        .execute(RunProtocolInput::new(
            ProtocolId::new("span-state-repeat").unwrap(),
            definition.clone(),
            ProtocolContext::empty(),
            LeaseOwnerId::new("span-host").unwrap(),
            DurationMs::from_millis(30_000),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();

    let definitions = Arc::new(InMemoryProtocolDefinitionRepository::new());
    let stream = stream();
    let clock = Arc::new(SystemClock::new());
    definitions.save(&definition).await.unwrap();
    StartProtocolUseCase::new(
        definitions.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    )
    .execute(StartProtocolInput::new(
        ProtocolId::new("span-delegated-repeat").unwrap(),
        definition.name().clone(),
        definition.version().clone(),
        ProtocolContext::empty(),
        "operator",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
    let delegated = RunProtocolStepUseCase::new(
        Arc::new(ResolveProtocolDefinitionUseCase::new(
            definitions,
            Arc::new(InMemoryProtocolDefinitionPublications::new()),
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
        state_iterations_for(&spans, "protocol_step", "span-state-repeat"),
        [1, 2]
    );
    assert_eq!(
        state_iterations_for(&spans, "run_protocol_step", "span-delegated-repeat"),
        [1, 2],
        "the first delegated span must retain the claimed state iteration even though completion opens the next one"
    );
}

fn repeating_step_input(protocol_id: &str, invocation: usize) -> RunProtocolStepInput {
    RunProtocolStepInput::new(
        ProtocolId::new(protocol_id).unwrap(),
        RoleId::new("SYSTEM").unwrap(),
        AuditActorKind::Service,
        StepId::new("work").unwrap(),
        LeaseOwnerId::new("span-host").unwrap(),
        IdempotencyKey::new(format!("{protocol_id}:work:{invocation}")).unwrap(),
        DurationMs::from_millis(30_000),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn normalized_handler_failure_and_explicit_step_parent_are_exported() {
    let (exporter, _guard) = install_bridge();
    let failed_one_shot = RunProtocolUseCase::new(
        Arc::new(InMemoryProtocolDefinitionRepository::new()),
        stream(),
        Arc::new(FailingHandler),
        Arc::new(SystemClock::new()),
    );
    assert!(failed_one_shot
        .execute(input("span-one-shot-failed"))
        .await
        .is_err());

    let definitions = Arc::new(InMemoryProtocolDefinitionRepository::new());
    let stream = stream();
    let clock = Arc::new(SystemClock::new());
    let definition = ProtocolDefinitionYaml::parse_str(LINEAR_PROTOCOL).unwrap();
    definitions.save(&definition).await.unwrap();
    StartProtocolUseCase::new(
        definitions.clone(),
        stream.clone(),
        clock.clone(),
        Arc::new(ForgetfulMemory::new()),
    )
    .execute(StartProtocolInput::new(
        ProtocolId::new("span-explicit").unwrap(),
        definition.name().clone(),
        definition.version().clone(),
        ProtocolContext::empty(),
        "operator",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
    let resolver = Arc::new(ResolveProtocolDefinitionUseCase::new(
        definitions,
        Arc::new(InMemoryProtocolDefinitionPublications::new()),
    ));
    let output = RunProtocolStepUseCase::new(resolver, stream, Arc::new(FailingHandler), clock)
        .execute(RunProtocolStepInput::new(
            ProtocolId::new("span-explicit").unwrap(),
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
    let failed_step = span_for(&spans, "protocol_step", "span-one-shot-failed");
    let failed_handler = span_for(&spans, "protocol_step_handler", "span-one-shot-failed");
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

    let run = span_for(&spans, "run_protocol_step", "span-explicit");
    let handler = span_for(&spans, "protocol_step_handler", "span-explicit");
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
