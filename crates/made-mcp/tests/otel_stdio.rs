#![cfg(all(feature = "embedded", feature = "otel"))]

use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use made_core::value_objects::AuthorizationAction;
use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::{
    TraceService, TraceServiceServer,
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_stream::wrappers::TcpListenerStream;
use tonic_otel::{Request, Response, Status};

#[path = "support/protected_embedded_stdio.rs"]
mod protected_stdio;

const CEREMONY_YAML: &str = r#"
version: "1.0"
name: otel_stdio_smoke
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
    handler: embedded_noop
guards:
  work_completed:
    type: automated
    check: "step_status:work:COMPLETED"
roles:
  - id: SYSTEM
    allowed_actions: [work, finish]
"#;

#[derive(Debug, Clone, Default)]
struct Collector {
    requests: Arc<Mutex<Vec<ExportTraceServiceRequest>>>,
}

#[tonic_otel::async_trait]
impl TraceService for Collector {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        self.requests.lock().unwrap().push(request.into_inner());
        Ok(Response::new(ExportTraceServiceResponse::default()))
    }
}

fn tool_call(id: u64, name: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    })
}

async fn start_collector() -> (Collector, std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let collector = Collector::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_collector = collector.clone();
    let server = tokio::spawn(async move {
        tonic_otel::transport::Server::builder()
            .add_service(TraceServiceServer::new(server_collector))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    (collector, address, server)
}

fn requests() -> [Value; 2] {
    let start_arguments = json!({
        "ceremony_id": "otel-stdio",
        "definition_yaml": CEREMONY_YAML,
        "actor_id": "operator",
        "actor_kind": "service",
        "context": {}
    });
    let step_arguments = json!({
        "ceremony_id": "otel-stdio",
        "step_id": "work",
        "actor_kind": "agent"
    });
    [
        tool_call(1, "made_start_ceremony", &start_arguments),
        tool_call(2, "made_run_ceremony_step", &step_arguments),
    ]
}

async fn run_mcp(address: std::net::SocketAddr) -> (Vec<String>, String) {
    let state = tempfile::tempdir().unwrap();
    let mut child = protected_stdio::command(
        state.path(),
        &[
            AuthorizationAction::StartCeremony,
            AuthorizationAction::RunCeremonyStep,
        ],
    )
    .await
    .env("MADE_OTLP_ENDPOINT", format!("http://{address}"))
    .env("RUST_LOG", "made_mcp=info,made_app=info,made_adapters=info")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true)
    .spawn()
    .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    for request in requests() {
        stdin
            .write_all(request.to_string().as_bytes())
            .await
            .unwrap();
        stdin.write_all(b"\n").await.unwrap();
    }
    stdin.flush().await.unwrap();
    drop(stdin);

    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut responses = Vec::new();
    for _ in 0..2 {
        responses.push(
            timeout(Duration::from_secs(10), stdout.next_line())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
        );
    }
    for line in &responses {
        let response: Value = serde_json::from_str(line).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_ne!(response["result"]["isError"], true);
    }

    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .await
        .unwrap();
    let status = timeout(Duration::from_secs(10), child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success(), "made-mcp failed: {stderr}");
    assert!(
        stderr.contains("otlp exporter wired"),
        "missing OTLP log: {stderr}"
    );
    assert!(
        stderr.contains("using durable embedded SQLite"),
        "startup diagnostics did not stay on stderr: {stderr}"
    );
    (responses, stderr)
}

fn assert_collected(collector: &Collector) {
    let requests = collector.requests.lock().unwrap();
    let mut names = Vec::new();
    let mut service_names = Vec::new();
    for request in requests.iter() {
        for resource_spans in &request.resource_spans {
            if let Some(resource) = &resource_spans.resource {
                for attribute in &resource.attributes {
                    if attribute.key == "service.name" {
                        if let Some(value) = &attribute.value {
                            if let Some(
                                opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(
                                    name,
                                ),
                            ) = &value.value
                            {
                                service_names.push(name.clone());
                            }
                        }
                    }
                }
            }
            for scope_spans in &resource_spans.scope_spans {
                names.extend(scope_spans.spans.iter().map(|span| span.name.clone()));
            }
        }
    }
    assert!(service_names.iter().any(|name| name == "made-mcp"));
    assert!(
        names.iter().any(|name| name == "run_ceremony_step"),
        "{names:?}"
    );
    assert!(
        names.iter().any(|name| name == "ceremony_step_handler"),
        "{names:?}"
    );
}

#[tokio::test]
async fn stdio_binary_exports_real_step_spans_without_polluting_stdout() {
    let (collector, address, server) = start_collector().await;
    let (responses, stderr) = run_mcp(address).await;
    assert_eq!(responses.len(), 2);
    assert!(stderr.contains("otlp exporter wired"));
    assert_collected(&collector);
    server.abort();
}
