//! Minimal stub of an OpenAI-compatible Chat Completions endpoint
//! for the stack E2E.
//!
//! Wired into `tests/e2e/docker-compose.e2e.yaml` as the `stub-llm`
//! sidecar so MADE's `OpenAiAgent` adapter has a real
//! HTTP peer to talk to — one that always returns a JSON Report
//! payload satisfying
//! `api/examples/output-contracts/report.schema.json`. That lets the
//! compose E2E exercise the positive structured-output path
//! (`RunCouncilDecision` with a strict Report contract -> winner
//! present + `validation.passed = true`) without bringing a real
//! provider into the repo's test path.
//!
//! Routes:
//!
//! - `POST /v1/chat/completions` — returns a deterministic
//!   chat-completion envelope. `choices[0].message.content` is a
//!   JSON string that, when parsed, satisfies the canonical Report
//!   schema. The request body is consumed but otherwise ignored —
//!   the stub does not branch on `model`, `messages`, or
//!   `max_tokens`.
//! - `GET /health` — `{"status":"ok"}` for compose-level probes.
//!
//! Environment:
//!
//! - `STUB_LLM_LISTEN` (default `0.0.0.0:8000`) — listen address.
//! - `RUST_LOG` — picked up by `tracing_subscriber::fmt`.

use std::env;
use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::extract::Json as JsonExtractor;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// Hard-coded report id baked into every response. Deterministic on
/// purpose so e2e assertions can compare equality across runs.
const STUB_REPORT_ID: &str = "stub-report-1";

/// Request body we accept on `/v1/chat/completions`. We deserialize
/// loosely (every field is optional) so the stub does not couple to
/// the real OpenAI shape beyond what the adapter actually sends —
/// the body is logged for diagnostics and otherwise ignored.
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
struct ChatCompletionsRequest {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    messages: Vec<Value>,
    #[serde(default)]
    max_tokens: Option<u32>,
}

/// Reduced OpenAI response shape. Only the fields the adapter reads
/// (`choices[0].message.content`) carry meaning; the rest are
/// populated with stable placeholders so the envelope looks well-
/// formed to operators inspecting compose logs.
#[derive(Serialize)]
struct ChatCompletionsResponse {
    id: &'static str,
    object: &'static str,
    created: u64,
    model: String,
    choices: Vec<ChatChoice>,
    usage: ChatUsage,
}

#[derive(Serialize)]
struct ChatChoice {
    index: u32,
    message: ChatMessage,
    finish_reason: &'static str,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    /// JSON-encoded Report payload. The OpenAI Chat Completions
    /// contract requires `content` to be a string; the
    /// `OpenAiAgent::generate` path returns whatever string lands
    /// here as the proposal content, and the JSON-Schema validator
    /// downstream parses that string as JSON.
    content: String,
}

// Clippy flags every-field-`_tokens` as a struct_field_names smell.
// Here the postfix is dictated by the OpenAI Chat Completions wire
// contract — the response field names are `prompt_tokens`,
// `completion_tokens`, `total_tokens`. Renaming would break the
// shape MADE's adapter expects.
#[allow(clippy::struct_field_names)]
#[derive(Serialize)]
struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// Build a Report payload that satisfies
/// `api/examples/output-contracts/report.schema.json`. The schema's
/// required fields are `report_id`, `executive_summary`, `findings`
/// (>=1), and `recommended_actions` (>=1). We also include
/// `evidence_references` so the stub exercises a non-trivial
/// optional branch — the validator must keep accepting that.
fn stub_report_payload() -> Value {
    json!({
        "report_id": STUB_REPORT_ID,
        "executive_summary": "Stub council report emitted by the MADE's compose E2E stub-llm sidecar.",
        "findings": [
            {
                "id": "finding-1",
                "summary": "Stub finding emitted by the stub-llm sidecar; carries no domain meaning.",
                "confidence": "medium"
            }
        ],
        "recommended_actions": [
            {
                "id": "action-1",
                "summary": "No-op recommendation; the stub exists to exercise contract-validated output end-to-end.",
                "approval_required": false
            }
        ],
        "evidence_references": [
            {
                "reference_id": "ref-1",
                "title": "stub-llm sidecar"
            }
        ]
    })
}

/// Return the canned Report-shaped chat completion. The handler
/// ignores the request body beyond logging a one-line diagnostic.
async fn chat_completions(
    JsonExtractor(req): JsonExtractor<ChatCompletionsRequest>,
) -> (StatusCode, Json<ChatCompletionsResponse>) {
    info!(
        model = req.model.as_deref().unwrap_or("<unset>"),
        messages = req.messages.len(),
        max_tokens = req.max_tokens.unwrap_or(0),
        "stub-llm: POST /v1/chat/completions"
    );
    let content = serde_json::to_string(&stub_report_payload())
        .expect("hard-coded JSON payload always serialises");
    let response = ChatCompletionsResponse {
        id: "chatcmpl-stub-1",
        object: "chat.completion",
        created: 1_700_000_000,
        model: req.model.unwrap_or_else(|| "stub-report-v1".to_owned()),
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant",
                content,
            },
            finish_reason: "stop",
        }],
        usage: ChatUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    };
    (StatusCode::OK, Json(response))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// Build the axum router. Split out so tests can mount the same
/// routes without binding a socket.
fn router() -> Router {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/health", get(health))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .compact()
        .init();

    let addr_raw = env::var("STUB_LLM_LISTEN").unwrap_or_else(|_| "0.0.0.0:8000".to_owned());
    let addr: SocketAddr = addr_raw
        .parse()
        .with_context(|| format!("invalid STUB_LLM_LISTEN: {addr_raw}"))?;

    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("stub-llm: failed to bind {addr}"))?;
    info!(%addr, "stub-llm listening");

    if let Err(err) = axum::serve(listener, router()).await {
        warn!(error = %err, "stub-llm: axum::serve returned an error");
        return Err(err).context("stub-llm: axum::serve terminated");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    /// The chat-completions route returns a well-formed envelope
    /// whose `choices[0].message.content` is a JSON string parsing
    /// to an object with every Report-schema required field.
    #[tokio::test]
    async fn chat_completions_returns_report_shaped_content() {
        let app = router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "model": "stub-report-v1",
                            "messages": [
                                {"role": "system", "content": "you are an agent"},
                                {"role": "user", "content": "draft a report"}
                            ],
                            "max_tokens": 256
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let envelope: Value = serde_json::from_slice(&bytes).unwrap();
        let content = envelope["choices"][0]["message"]["content"]
            .as_str()
            .expect("choices[0].message.content must be a string");
        let payload: Value = serde_json::from_str(content).expect("content must parse as JSON");
        for required in [
            "report_id",
            "executive_summary",
            "findings",
            "recommended_actions",
        ] {
            assert!(
                payload.get(required).is_some(),
                "report payload missing required field {required}: {payload}"
            );
        }
        assert_eq!(
            payload["report_id"],
            serde_json::Value::from(STUB_REPORT_ID)
        );
    }

    /// The health route returns the `{"status":"ok"}` body operators
    /// expect from compose probes.
    #[tokio::test]
    async fn health_returns_ok_status() {
        let app = router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["status"], "ok");
    }
}
