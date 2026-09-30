//! Real-server container integration test for the MCP adapter.
//!
//! Spins up a published MADE image via `testcontainers`,
//! spawns the locally-built `made-mcp` binary in gRPC mode pointing
//! at the container's mapped port, and exercises the JSON-RPC surface:
//!
//!   1. `tools/list` — require the same executable surface returned by
//!      machine-readable discovery, including server-owned guidance.
//!   2. `tools/call` — invoke the 4 simplest read-only tools and
//!      assert the response is a well-formed JSON-RPC envelope with
//!      a `result` object.
//!
//! The per-tool 1:1 input/output shape is already covered by the
//! fixture-backed unit tests in `crates/made-mcp/src/`. The
//! invariant we want here is the end-to-end wiring:
//!
//!     MCP stdio (binary) ↔ gRPC client ↔ real MADE
//!
//! Pulls the published `:latest` image instead of building from the
//! repo's `Dockerfile` because `testcontainers` does not expose a
//! "docker build" primitive in its 0.23 series — running the build
//! out-of-band would dilute the contract this test claims.
//! `MADE_REAL_KERNEL_IMAGE=<name>:<tag>` points it at another image, such
//! as one built locally for a platform the published image lacks.
//!
//! The server only serves behind mutual TLS with an authorization policy,
//! so the test prepares both the way the compose E2E does: the fixture
//! certificates and principal map from `tests/e2e/prepare-auth.sh`, and
//! the policy from the image's own `bootstrap-authorization` over the
//! store the server then opens.
//!
//! Gated on the `container-tests` feature. Workspace-default
//! `cargo test --workspace` skips this file and stays fast +
//! network-free; the `integration-image` CI job runs it through
//! `scripts/ci/integration-image.sh`.

#![cfg(feature = "container-tests")]

use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use testcontainers::{
    core::{AccessMode, IntoContainerPort, Mount, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

const MADE_IMAGE: &str = "ghcr.io/underpass-ai/made";
const MADE_TAG: &str = "latest";
const MADE_IMAGE_ENV: &str = "MADE_REAL_KERNEL_IMAGE";
const MADE_GRPC_PORT: u16 = 50055;
const MADE_HTTP_PORT: u16 = 8080;

// The names `tests/e2e/prepare-auth.sh` issues: the server certificate is
// for `made`, the client certificate is the trusted host `made-e2e-owner`.
const SERVER_NAME: &str = "made";
const TRUSTED_HOST: &str = "made-e2e-owner";
const POLICY_ID: &str = "made-real-kernel-policy";
const AUTH_DIR: &str = "/etc/made/auth";
const STATE_DIR: &str = "/var/lib/made";
const STORE_PATH: &str = "/var/lib/made/made.sqlite3";

/// Newline-delimited JSON-RPC request/response helper.
struct McpStdio {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    line_buf: String,
    next_id: u64,
}

impl McpStdio {
    async fn call(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_string(&req).expect("serialize request");
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .expect("write stdin");
        self.stdin.flush().await.expect("flush stdin");

        self.line_buf.clear();
        let read_fut = self.stdout.read_line(&mut self.line_buf);
        let bytes = timeout(Duration::from_secs(20), read_fut)
            .await
            .expect("MCP response within 20s")
            .expect("read stdout");
        assert!(bytes > 0, "MCP closed stdout before responding");
        serde_json::from_str(self.line_buf.trim_end())
            .unwrap_or_else(|err| panic!("malformed JSON-RPC response: {err}: {}", self.line_buf))
    }
}

/// Host directories the server container mounts: the mTLS fixture and the
/// store. Both go away with the test.
struct KernelFiles {
    _root: tempfile::TempDir,
    auth: PathBuf,
    state: PathBuf,
}

impl KernelFiles {
    fn prepare() -> Self {
        let root = tempfile::tempdir().expect("scratch directory");
        let auth = root.path().join("auth");
        let state = root.path().join("state");
        let script =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/prepare-auth.sh");
        let status = std::process::Command::new("bash")
            .arg(&script)
            .arg(&auth)
            .status()
            .expect("run prepare-auth.sh");
        assert!(status.success(), "prepare-auth.sh failed: {status}");
        std::fs::create_dir(&state).expect("state directory");
        // The image runs as distroless `nonroot`; the scratch store must be
        // writable by it.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o777))
                .expect("open the state directory to the container user");
        }
        Self {
            _root: root,
            auth,
            state,
        }
    }

    fn host_path(&self, file: &str) -> PathBuf {
        self.auth.join(file)
    }
}

fn made_image() -> GenericImage {
    let reference = env::var(MADE_IMAGE_ENV).unwrap_or_else(|_| format!("{MADE_IMAGE}:{MADE_TAG}"));
    let (name, tag) = reference
        .rsplit_once(':')
        .unwrap_or_else(|| panic!("{MADE_IMAGE_ENV} must be <name>:<tag>, got `{reference}`"));
    GenericImage::new(name, tag)
}

fn with_store(
    image: GenericImage,
    files: &KernelFiles,
) -> testcontainers::core::ContainerRequest<GenericImage> {
    image
        .with_mount(Mount::bind_mount(
            files.state.display().to_string(),
            STATE_DIR,
        ))
        .with_env_var("MADE_CEREMONY_STORE_PATH", STORE_PATH)
}

/// Open the authorization policy in the store before the server needs it,
/// with the image's own one-shot command, as the compose E2E does.
async fn bootstrap_authorization(files: &KernelFiles) {
    let _bootstrap = with_store(
        made_image().with_wait_for(WaitFor::message_on_stdout("\"policy_id\"")),
        files,
    )
    .with_cmd([
        "bootstrap-authorization",
        "--policy-id",
        POLICY_ID,
        "--trusted-host-id",
        TRUSTED_HOST,
    ])
    .start()
    .await
    .expect("bootstrap-authorization should open the policy");
}

async fn spawn_made(files: &KernelFiles) -> testcontainers::ContainerAsync<GenericImage> {
    bootstrap_authorization(files).await;
    with_store(
        made_image()
            .with_exposed_port(MADE_GRPC_PORT.tcp())
            .with_exposed_port(MADE_HTTP_PORT.tcp())
            // The binary writes the readiness line via tracing JSON. Wait
            // for the gRPC bind log so we don't race the dial-in. Looser
            // pattern keeps the test resilient to tracing format tweaks.
            .with_wait_for(WaitFor::message_on_stdout("servers starting")),
        files,
    )
    .with_mount(
        Mount::bind_mount(files.auth.display().to_string(), AUTH_DIR)
            .with_access_mode(AccessMode::ReadOnly),
    )
    .with_env_var("MADE_GRPC_PORT", MADE_GRPC_PORT.to_string())
    .with_env_var("MADE_HTTP_PORT", MADE_HTTP_PORT.to_string())
    .with_env_var("MADE_GRPC_TLS_MODE", "mutual")
    .with_env_var("MADE_GRPC_TLS_CERT_PATH", format!("{AUTH_DIR}/server.pem"))
    .with_env_var("MADE_GRPC_TLS_KEY_PATH", format!("{AUTH_DIR}/server.key"))
    .with_env_var("MADE_GRPC_TLS_CLIENT_CA_PATH", format!("{AUTH_DIR}/ca.pem"))
    .with_env_var("MADE_AUTH_POLICY_ID", POLICY_ID)
    // made-mcp forwards the digest of the definition it authorizes against;
    // the server honours that header only from a principal it trusts as an
    // MCP proxy, which the MCP's client certificate is here.
    .with_env_var("MADE_AUTH_MCP_PROXY_PRINCIPAL_IDS", TRUSTED_HOST)
    .with_env_var(
        "MADE_AUTH_MTLS_PRINCIPALS_PATH",
        format!("{AUTH_DIR}/principals.json"),
    )
    // Public, fixture-only store identity and cursor key.
    .with_env_var("MADE_CEREMONY_STORE_ID", "made-real-kernel-store")
    .with_env_var("MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY", "a5".repeat(32))
    // Disable NATS so the test does not need a broker side-car.
    .with_env_var("MADE_NATS_ENABLED", "false")
    // Seed one council so `made_list_councils` returns a
    // non-empty list and the test exercises a real read path.
    .with_env_var("MADE_SEED_SPECIALTIES", "triage")
    .with_env_var("RUST_LOG", "info")
    .start()
    .await
    .expect("made container should start")
}

fn spawn_mcp(endpoint: &str, files: &KernelFiles) -> McpStdio {
    // Cargo builds the package's binary before its integration tests.
    let mut child = Command::new(env!("CARGO_BIN_EXE_made-mcp"))
        .env("MADE_MCP_BACKEND", "grpc")
        .env("MADE_MCP_GRPC_ENDPOINT", endpoint)
        .env("MADE_MCP_GRPC_TLS_MODE", "mutual")
        .env("MADE_MCP_GRPC_TLS_CA_PATH", files.host_path("ca.pem"))
        .env("MADE_MCP_GRPC_TLS_CERT_PATH", files.host_path("client.pem"))
        .env("MADE_MCP_GRPC_TLS_KEY_PATH", files.host_path("client.key"))
        .env("MADE_MCP_GRPC_TLS_DOMAIN_NAME", SERVER_NAME)
        .env("RUST_LOG", "made_mcp=info")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn made-mcp");

    let stdin_pipe = child.stdin.take().expect("child stdin");
    let stdout_pipe = child.stdout.take().expect("child stdout");
    let connection = McpStdio {
        stdin: stdin_pipe,
        stdout: BufReader::new(stdout_pipe),
        line_buf: String::new(),
        next_id: 0,
    };

    // Leak the child handle: the test process exits at the end of
    // the test and the OS reaps the spawned MCP binary. Killing it
    // explicitly would race the stdout drain.
    std::mem::forget(child);

    connection
}

#[tokio::test]
async fn mcp_lists_full_tool_catalog_and_calls_read_endpoints() {
    let files = KernelFiles::prepare();
    let container = spawn_made(&files).await;
    let port = container
        .get_host_port_ipv4(MADE_GRPC_PORT.tcp())
        .await
        .expect("host port");
    let endpoint = format!("https://127.0.0.1:{port}");

    let mut mcp = spawn_mcp(&endpoint, &files);

    // Initialize first — many clients require it, and the MCP
    // server exposes adapter-side metadata in the reply that we
    // log for debug if the next call fails.
    let init = mcp.call("initialize", json!({})).await;
    assert_eq!(
        init.get("jsonrpc").and_then(Value::as_str),
        Some("2.0"),
        "initialize: malformed envelope: {init:?}",
    );
    let init_result = init
        .get("result")
        .unwrap_or_else(|| panic!("initialize had no result: {init:?}"));
    assert!(
        init_result.get("serverInfo").is_some(),
        "initialize missing serverInfo: {init:?}",
    );

    // tools/list — assert the same executable surface as machine discovery,
    // avoiding another hard-coded cardinal that can drift from the proto.
    let list = mcp.call("tools/list", json!({})).await;
    let tools = list
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("tools/list missing /result/tools array: {list:?}"));
    let listed_names = tools
        .iter()
        .map(|tool| tool["name"].clone())
        .collect::<Vec<_>>();
    let discovery = mcp
        .call(
            "tools/call",
            json!({
                "name": "made_discover_capabilities",
                "arguments": {}
            }),
        )
        .await;
    let discovered_tools = discovery
        .pointer("/result/structuredContent/tools")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("discovery missing structured tools: {discovery:?}"));
    let discovered_names = discovered_tools
        .iter()
        .map(|tool| tool["name"].clone())
        .collect::<Vec<_>>();
    assert_eq!(listed_names, discovered_names);
    assert_eq!(
        discovery.pointer("/result/structuredContent/tool_count"),
        Some(&json!(tools.len()))
    );
    assert!(listed_names.contains(&json!("made_discover_capabilities")));
    assert!(listed_names.contains(&json!("made_get_help")));
    let observability = discovery
        .pointer("/result/structuredContent/capabilities")
        .and_then(Value::as_array)
        .and_then(|groups| {
            groups
                .iter()
                .find(|group| group["id"] == "service_observability")
        })
        .unwrap_or_else(|| panic!("discovery missing service_observability: {discovery:?}"));
    assert_eq!(
        observability["tools"],
        json!(["made_get_status", "made_get_metrics"])
    );

    // Sanity-check that every tool name starts with `made_`.
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("tool entry missing name: {tool:?}"));
        assert!(name.starts_with("made_"), "tool name not prefixed: {name}");
    }

    assert_simple_read_tools(&mut mcp).await;
}

async fn assert_simple_read_tools(mcp: &mut McpStdio) {
    // Each read should come back as a well-formed JSON-RPC envelope with a
    // result object.
    let simple_tools = [
        "made_list_councils",
        "made_list_contracts",
        "made_get_metrics",
        "made_get_status",
    ];
    grant_to_trusted_host(mcp, &simple_tools).await;

    for tool in simple_tools {
        let resp = mcp
            .call(
                "tools/call",
                json!({
                    "name": tool,
                    "arguments": {},
                }),
            )
            .await;
        assert_eq!(
            resp.get("jsonrpc").and_then(Value::as_str),
            Some("2.0"),
            "{tool}: malformed envelope: {resp:?}",
        );
        let result = resp
            .get("result")
            .unwrap_or_else(|| panic!("{tool}: missing result: {resp:?}"));
        assert!(
            result.is_object(),
            "{tool}: result is not an object: {result:?}",
        );
        // Per MCP spec, tool errors come back as `isError: true`
        // inside `result`, not as a JSON-RPC `error`. A real failure
        // here means the gRPC wiring or MADE dropped
        // the call — every read tool should land cleanly.
        let is_error = result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        assert!(!is_error, "{tool}: returned isError=true: {result:?}");
    }
}

/// The bootstrapped policy names the trusted host its owner, and an owner
/// administers authority rather than holding it: reading needs an explicit
/// grant, issued here through the MCP surface itself as a host would.
async fn grant_to_trusted_host(mcp: &mut McpStdio, tools: &[&str]) {
    let actions = tools
        .iter()
        .map(|tool| tool.strip_prefix("made_").expect("made_ tool"))
        .collect::<Vec<_>>();
    let issued = mcp
        .call(
            "tools/call",
            json!({
                "name": "made_issue_authorization_grant",
                "arguments": {
                    "grant_id": "made-real-kernel-reads",
                    "grantee_id": TRUSTED_HOST,
                    "actions": actions,
                    "scope": {"kind": "global"},
                    "valid_from": "1970-01-01T00:00:00Z",
                    "delegation_depth": 0
                }
            }),
        )
        .await;
    assert_eq!(
        issued.pointer("/result/isError"),
        Some(&json!(false)),
        "grant for the read tools was refused: {issued:?}",
    );
}
