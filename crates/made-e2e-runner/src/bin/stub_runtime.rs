//! Minimal stub of `underpass.runtime.v1.SessionService` +
//! `InvocationService` for stack E2E.
//!
//! Wired into `tests/e2e/docker-compose.e2e.yaml` as the `stub-runtime`
//! sidecar so MADE's `RuntimeExecutor` has a real gRPC
//! peer to talk to without dragging the actual `underpass-runtime`
//! image into this repo's test path.
//!
//! Every RPC returns a deterministic canned response:
//!   - `CreateSession` → `{ id: "stub-session-1", ... }`
//!   - `InvokeTool`    → `{ id: "stub-invocation-1", status: SUCCEEDED, ... }`
//!   - `CloseSession`  → `{ closed: true }`
//!
//! `STUB_RUNTIME_GRPC_ADDR` (default `0.0.0.0:50053`) controls the
//! listener.

use std::env;
use std::net::SocketAddr;

use anyhow::{Context, Result};
use made_proto::runtime_v1 as rt;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Default, Clone)]
struct StubRuntime;

#[tonic::async_trait]
impl rt::session_service_server::SessionService for StubRuntime {
    async fn create_session(
        &self,
        request: Request<rt::CreateSessionRequest>,
    ) -> Result<Response<rt::CreateSessionResponse>, Status> {
        let request = request.into_inner();
        info!(
            requested_session_id = request.session_id.as_str(),
            "stub-runtime: CreateSession"
        );
        Ok(Response::new(rt::CreateSessionResponse {
            session: Some(rt::Session {
                id: "stub-session-1".to_owned(),
                workspace_path: "/tmp/stub-workspace".to_owned(),
                runtime: None,
                repo_url: request.repo_url,
                repo_ref: request.repo_ref,
                allowed_paths: request.allowed_paths,
                principal: request.principal,
                metadata: request.metadata,
                created_at: None,
                expires_at: None,
            }),
        }))
    }

    async fn close_session(
        &self,
        request: Request<rt::CloseSessionRequest>,
    ) -> Result<Response<rt::CloseSessionResponse>, Status> {
        let request = request.into_inner();
        info!(
            session_id = request.session_id.as_str(),
            "stub-runtime: CloseSession"
        );
        Ok(Response::new(rt::CloseSessionResponse { closed: true }))
    }
}

#[tonic::async_trait]
impl rt::invocation_service_server::InvocationService for StubRuntime {
    async fn invoke_tool(
        &self,
        request: Request<rt::InvokeToolRequest>,
    ) -> Result<Response<rt::InvokeToolResponse>, Status> {
        let request = request.into_inner();
        info!(
            session_id = request.session_id.as_str(),
            tool_name = request.tool_name.as_str(),
            correlation_id = request.correlation_id.as_str(),
            "stub-runtime: InvokeTool"
        );
        Ok(Response::new(rt::InvokeToolResponse {
            invocation: Some(rt::Invocation {
                id: "stub-invocation-1".to_owned(),
                session_id: request.session_id,
                tool_name: request.tool_name,
                correlation_id: request.correlation_id,
                status: rt::InvocationStatus::Succeeded as i32,
                started_at: None,
                completed_at: None,
                duration_ms: 5,
                trace_name: String::new(),
                span_name: String::new(),
                exit_code: 0,
                output: None,
                output_ref: String::new(),
                logs: Vec::new(),
                logs_ref: String::new(),
                artifacts: Vec::new(),
                error: None,
            }),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .compact()
        .init();

    let addr_raw =
        env::var("STUB_RUNTIME_GRPC_ADDR").unwrap_or_else(|_| "0.0.0.0:50053".to_string());
    let addr: SocketAddr = addr_raw
        .parse()
        .with_context(|| format!("invalid STUB_RUNTIME_GRPC_ADDR: {addr_raw}"))?;
    info!(%addr, "stub-runtime listening");

    let stub = StubRuntime;
    Server::builder()
        .add_service(rt::session_service_server::SessionServiceServer::new(
            stub.clone(),
        ))
        .add_service(rt::invocation_service_server::InvocationServiceServer::new(
            stub,
        ))
        .serve(addr)
        .await
        .context("stub-runtime gRPC server terminated with error")?;
    Ok(())
}
