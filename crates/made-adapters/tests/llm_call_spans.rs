#![cfg(all(
    feature = "agent-openai",
    feature = "agent-vllm",
    feature = "agent-anthropic",
    feature = "otel"
))]

use std::sync::Arc;

use made_adapters::agents::anthropic::{AnthropicAgent, AnthropicApiKey, AnthropicConfig};
use made_adapters::agents::judge::LlmJudgeValidator;
use made_adapters::agents::openai::{OpenAiAgent, OpenAiApiKey, OpenAiConfig};
use made_adapters::agents::support_judge::LlmEvidenceSupportJudge;
use made_adapters::agents::vllm::{VllmAgent, VllmConfig};
use made_core::entities::TaskConstraints;
use made_core::ports::{
    AgentPort, DraftRequest, EvidenceSupportJudgePort, NoopMetricsRecorder, ValidatorPort,
};
use made_core::value_objects::{
    AgentId, ClaimText, DiversityPreference, EvidenceBody, EvidenceExcerpt, EvidenceReference,
    Rounds, Rubric, Specialty, TaskDescription,
};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::Value;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SimpleSpanProcessor};
use serde_json::json;
use tracing::Instrument as _;
use tracing_subscriber::layer::SubscriberExt as _;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn install_bridge() -> (InMemorySpanExporter, tracing::subscriber::DefaultGuard) {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
        .build();
    let tracer = provider.tracer("llm-call-span-tests");
    let subscriber = tracing_subscriber::registry().with(
        tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_context_activation(false),
    );
    (exporter, tracing::subscriber::set_default(subscriber))
}

fn draft() -> DraftRequest {
    DraftRequest {
        task: TaskDescription::new("Inspect the observed condition").unwrap(),
        constraints: TaskConstraints::new(Rubric::empty(), Rounds::default(), None, None),
        diversity: DiversityPreference::Diverse,
        external_context: None,
    }
}

fn chat_response(text: &str, prompt: u32, completion: u32) -> serde_json::Value {
    json!({
        "choices": [{"message": {"content": text}}],
        "usage": {"prompt_tokens": prompt, "completion_tokens": completion}
    })
}

fn anthropic_response(text: &str, prompt: u32, completion: u32) -> serde_json::Value {
    json!({
        "content": [{"type": "text", "text": text}],
        "usage": {"input_tokens": prompt, "output_tokens": completion}
    })
}

async fn openai_agent(response: ResponseTemplate, model: &str) -> (MockServer, OpenAiAgent) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(response)
        .mount(&server)
        .await;
    let agent = OpenAiAgent::new(
        AgentId::new("openai-test").unwrap(),
        Specialty::new("review").unwrap(),
        OpenAiConfig::new(OpenAiApiKey::new("secret").unwrap())
            .with_endpoint(server.uri())
            .unwrap()
            .with_model(model)
            .unwrap(),
    )
    .unwrap();
    (server, agent)
}

async fn vllm_agent(response: ResponseTemplate, model: &str) -> (MockServer, VllmAgent) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(response)
        .mount(&server)
        .await;
    let agent = VllmAgent::new(
        AgentId::new(format!("{model}-test")).unwrap(),
        Specialty::new("review").unwrap(),
        VllmConfig::new(model)
            .unwrap()
            .with_endpoint(server.uri())
            .unwrap(),
    )
    .unwrap();
    (server, agent)
}

async fn anthropic_agent(response: ResponseTemplate, model: &str) -> (MockServer, AnthropicAgent) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(response)
        .mount(&server)
        .await;
    let agent = AnthropicAgent::new(
        AgentId::new(format!("{model}-test")).unwrap(),
        Specialty::new("review").unwrap(),
        AnthropicConfig::new(AnthropicApiKey::new("secret").unwrap())
            .with_endpoint(server.uri())
            .unwrap()
            .with_model(model)
            .unwrap(),
    )
    .unwrap();
    (server, agent)
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

fn find_span(
    spans: &[opentelemetry_sdk::trace::SpanData],
    name: &str,
    key: &str,
    value: &str,
) -> opentelemetry_sdk::trace::SpanData {
    spans
        .iter()
        .find(|span| span.name == name && string_attr(span, key).as_deref() == Some(value))
        .unwrap_or_else(|| panic!("no {name} span with {key}={value}"))
        .clone()
}

#[tokio::test(flavor = "current_thread")]
async fn provider_spans_export_identity_usage_failures_and_parentage() {
    let (exporter, _guard) = install_bridge();
    let (_openai_server, openai) = openai_agent(
        ResponseTemplate::new(200).set_body_json(chat_response("ok", 10, 4)),
        "gpt-test",
    )
    .await;
    let (_vllm_server, vllm) = vllm_agent(ResponseTemplate::new(429), "qwen-test").await;
    let (_vllm_success_server, vllm_success) = vllm_agent(
        ResponseTemplate::new(200).set_body_json(chat_response("ok", 11, 6)),
        "qwen-success",
    )
    .await;
    let (_anthropic_server, anthropic) = anthropic_agent(
        ResponseTemplate::new(200).set_body_json(anthropic_response(" ", 7, 2)),
        "claude-test",
    )
    .await;
    let (_anthropic_success_server, anthropic_success) = anthropic_agent(
        ResponseTemplate::new(200).set_body_json(anthropic_response("ok", 9, 3)),
        "claude-success",
    )
    .await;

    openai
        .generate(draft())
        .instrument(tracing::info_span!("deliberate.openai"))
        .await
        .unwrap();
    assert!(vllm.generate(draft()).await.is_err());
    assert!(anthropic.generate(draft()).await.is_err());
    vllm_success.generate(draft()).await.unwrap();
    anthropic_success.generate(draft()).await.unwrap();

    let spans = exporter.get_finished_spans().unwrap();
    let openai_span = find_span(&spans, "provider_call", "provider", "openai");
    assert_eq!(
        string_attr(&openai_span, "model").as_deref(),
        Some("gpt-test")
    );
    assert_eq!(
        string_attr(&openai_span, "operation").as_deref(),
        Some("generate")
    );
    assert_eq!(
        string_attr(&openai_span, "outcome").as_deref(),
        Some("success")
    );
    assert_eq!(
        int_attr(&openai_span, "prompt_tokens"),
        Some(10),
        "{:?}",
        openai_span.attributes
    );
    assert_eq!(int_attr(&openai_span, "completion_tokens"), Some(4));
    let parent = spans
        .iter()
        .find(|span| span.name == "deliberate.openai")
        .unwrap();
    assert_eq!(openai_span.parent_span_id, parent.span_context.span_id());

    let vllm_span = find_span(&spans, "provider_call", "model", "qwen-test");
    assert_eq!(string_attr(&vllm_span, "outcome").as_deref(), Some("error"));
    assert_eq!(
        string_attr(&vllm_span, "error_kind").as_deref(),
        Some("rate_limited")
    );
    assert_eq!(int_attr(&vllm_span, "prompt_tokens"), None);

    let vllm_success_span = find_span(&spans, "provider_call", "model", "qwen-success");
    assert_eq!(
        string_attr(&vllm_success_span, "provider").as_deref(),
        Some("vllm")
    );
    assert_eq!(
        string_attr(&vllm_success_span, "outcome").as_deref(),
        Some("success")
    );
    assert_eq!(int_attr(&vllm_success_span, "prompt_tokens"), Some(11));

    let anthropic_span = find_span(&spans, "provider_call", "model", "claude-test");
    assert_eq!(
        string_attr(&anthropic_span, "error_kind").as_deref(),
        Some("empty_content")
    );
    assert_eq!(int_attr(&anthropic_span, "prompt_tokens"), Some(7));
    assert_eq!(int_attr(&anthropic_span, "completion_tokens"), Some(2));

    let anthropic_success_span = find_span(&spans, "provider_call", "model", "claude-success");
    assert_eq!(
        string_attr(&anthropic_success_span, "provider").as_deref(),
        Some("anthropic")
    );
    assert_eq!(
        string_attr(&anthropic_success_span, "outcome").as_deref(),
        Some("success")
    );
    assert_eq!(int_attr(&anthropic_success_span, "prompt_tokens"), Some(9));
}

#[tokio::test(flavor = "current_thread")]
async fn judge_spans_export_concrete_identity_usage_and_malformed_usage() {
    let (exporter, _guard) = install_bridge();
    let quality_server = MockServer::start().await;
    let support_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_response(
            r#"{"score": 90}"#,
            20,
            3,
        )))
        .mount(&quality_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_response("not-json", 8, 5)))
        .mount(&support_server)
        .await;

    let quality = LlmJudgeValidator::new(
        quality_server.uri(),
        "judge-model",
        0.5,
        Arc::new(NoopMetricsRecorder),
    )
    .unwrap()
    .with_vllm_identity();
    quality
        .validate("proposal", &TaskConstraints::default())
        .instrument(tracing::info_span!("deliberate.quality"))
        .await
        .unwrap();

    let support = LlmEvidenceSupportJudge::new(
        support_server.uri(),
        "support-model",
        Arc::new(NoopMetricsRecorder),
    )
    .unwrap()
    .with_vllm_identity();
    let evidence = [EvidenceExcerpt::new(
        EvidenceReference::new("ev-1").unwrap(),
        EvidenceBody::new("observed evidence").unwrap(),
    )];
    assert!(support
        .assess(&ClaimText::new("claim").unwrap(), &evidence)
        .await
        .is_err());

    let spans = exporter.get_finished_spans().unwrap();
    let quality_span = find_span(&spans, "judge_call", "judge_kind", "quality");
    assert_eq!(
        string_attr(&quality_span, "provider").as_deref(),
        Some("vllm")
    );
    assert_eq!(
        string_attr(&quality_span, "model").as_deref(),
        Some("judge-model")
    );
    assert_eq!(
        string_attr(&quality_span, "outcome").as_deref(),
        Some("success")
    );
    assert_eq!(
        int_attr(&quality_span, "prompt_tokens"),
        Some(20),
        "{:?}",
        quality_span.attributes
    );
    let quality_parent = spans
        .iter()
        .find(|span| span.name == "deliberate.quality")
        .unwrap();
    assert_eq!(
        quality_span.parent_span_id,
        quality_parent.span_context.span_id()
    );

    let support_span = find_span(&spans, "judge_call", "judge_kind", "evidence_support");
    assert_eq!(
        string_attr(&support_span, "provider").as_deref(),
        Some("vllm")
    );
    assert_eq!(
        string_attr(&support_span, "outcome").as_deref(),
        Some("error")
    );
    assert_eq!(
        string_attr(&support_span, "error_kind").as_deref(),
        Some("malformed_body")
    );
    assert_eq!(int_attr(&support_span, "prompt_tokens"), Some(8));
    assert_eq!(int_attr(&support_span, "completion_tokens"), Some(5));
}
