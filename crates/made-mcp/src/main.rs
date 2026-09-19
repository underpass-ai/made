//! `made-mcp` — stdio MCP adapter for MADE.
//!
//! Reads one JSON-RPC line at a time from stdin, dispatches to the inner
//! [`MadeMcpServer`], and writes responses to stdout. Logs go to stderr.

use std::io::{self, BufRead, Write};

use made_mcp::{
    MadeMcpServer, EMBEDDED_STORE_PATH_ENV, GRPC_ENDPOINT_ENV, GRPC_TLS_CA_PATH_ENV,
    GRPC_TLS_CERT_PATH_ENV, GRPC_TLS_DOMAIN_NAME_ENV, GRPC_TLS_KEY_PATH_ENV, GRPC_TLS_MODE_ENV,
    MCP_BACKEND_ENV,
};
#[cfg(not(feature = "otel"))]
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _telemetry = init_tracing()?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(command) = args.first() {
        std::process::exit(run_cli_command(command, &args[1..]).await);
    }

    let server = match MadeMcpServer::try_from_env() {
        Ok(server) => server,
        Err(message) => {
            eprintln!("made-mcp: {message}");
            if message.contains(MCP_BACKEND_ENV) {
                eprintln!("made-mcp: select a compiled backend with {MCP_BACKEND_ENV}");
            }
            std::process::exit(2);
        }
    };

    if server.backend_name() == "grpc" {
        eprintln!(
            "made-mcp: using live gRPC backend from {GRPC_ENDPOINT_ENV} with {GRPC_TLS_MODE_ENV}={}",
            server.grpc_tls_mode_name()
        );
        if server.grpc_tls_mode_name() != "disabled" {
            eprintln!(
                "made-mcp: TLS envs: {GRPC_TLS_CA_PATH_ENV}, {GRPC_TLS_CERT_PATH_ENV}, {GRPC_TLS_KEY_PATH_ENV}, {GRPC_TLS_DOMAIN_NAME_ENV}"
            );
        }
    } else if server.backend_name() == "embedded" {
        eprintln!(
            "made-mcp: using durable embedded SQLite ceremony backend from {EMBEDDED_STORE_PATH_ENV}"
        );
    } else {
        eprintln!("made-mcp: using explicit fixture backend");
    }

    if let Err(message) = server.initialize_backend().await {
        eprintln!("made-mcp: {message}");
        std::process::exit(2);
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = server.handle_json_line(&line).await {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }

    Ok(())
}

#[cfg(not(feature = "otel"))]
#[derive(Debug)]
struct McpTelemetryGuard;

#[cfg(feature = "otel")]
type McpTelemetryGuard = made_adapters::telemetry::TelemetryGuard;

#[cfg(not(feature = "otel"))]
#[allow(clippy::unnecessary_wraps)] // keeps the call site identical to the fallible otel build
fn init_tracing() -> anyhow::Result<McpTelemetryGuard> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("made_mcp=info,made_app=info,made_adapters::sqlite=info")
    });
    tracing_subscriber::fmt()
        .json()
        .with_writer(io::stderr)
        .with_env_filter(filter)
        .init();
    Ok(McpTelemetryGuard)
}

#[cfg(feature = "otel")]
fn init_tracing() -> anyhow::Result<McpTelemetryGuard> {
    made_adapters::telemetry::init_otlp_tracing(
        "made-mcp",
        env!("CARGO_PKG_VERSION"),
        "made_mcp=info,made_app=info,made_adapters::sqlite=info",
        io::stderr,
    )
}

async fn run_cli_command(command: &str, args: &[String]) -> i32 {
    match command {
        "--version" | "-V" | "version" if args.is_empty() => {
            println!(
                "made-mcp {} ({})",
                env!("CARGO_PKG_VERSION"),
                embedded_store_carried()
            );
            0
        }
        "migrate-store" => {
            if let [path] = args {
                run_migrate_store(path).await
            } else {
                eprintln!("made-mcp: usage: made-mcp migrate-store <path-to-ceremony-store>");
                2
            }
        }
        "bootstrap-authorization" => match args {
            [path, policy_flag, policy_id, host_flag, trusted_host_id]
                if policy_flag == "--policy-id" && host_flag == "--trusted-host-id" =>
            {
                run_bootstrap_authorization(path, policy_id, trusted_host_id).await
            }
            _ => {
                eprintln!(
                        "made-mcp: usage: made-mcp bootstrap-authorization <store> --policy-id <id> --trusted-host-id <id>"
                    );
                2
            }
        },
        other => {
            eprintln!(
                "made-mcp: unknown command `{other}`; run without arguments for MCP stdio mode, or use `--version`, `migrate-store <path>`, or `bootstrap-authorization <store> --policy-id <id> --trusted-host-id <id>`"
            );
            2
        }
    }
}

#[cfg(feature = "embedded")]
async fn run_bootstrap_authorization(path: &str, policy_id: &str, trusted_host_id: &str) -> i32 {
    use std::sync::Arc;

    use made_adapters::clock::SystemClock;
    use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
    use made_app::authorization::{
        AuthorizationMutationOutcome, AuthorizationPolicyAdministrationService,
    };
    use made_core::value_objects::{
        AuthenticatedPrincipal, AuthenticationMethod, AuthorizationPolicyId, PrincipalId,
        PrincipalKind,
    };

    let outcome = async {
        let policy_id = AuthorizationPolicyId::new(policy_id)?;
        let owner = AuthenticatedPrincipal::new(
            PrincipalId::new(trusted_host_id)?,
            PrincipalKind::TrustedHost,
            AuthenticationMethod::LocalHostPolicy,
        )?;
        let store = Arc::new(SqliteAuthorizationPolicyStore::open(path)?);
        AuthorizationPolicyAdministrationService::new(
            policy_id,
            store,
            Arc::new(SystemClock::new()),
        )
        .open(owner, Vec::new())
        .await
    }
    .await;
    match outcome {
        Ok(AuthorizationMutationOutcome::Applied { version }) => {
            println!("authorization policy opened at version {}", version.value());
            println!(
                "the trusted-host owner may administer policy; issue explicit grants before serving protected operations"
            );
            0
        }
        Ok(AuthorizationMutationOutcome::Existing { version }) => {
            println!(
                "authorization policy already exists at version {}",
                version.value()
            );
            println!(
                "the trusted-host owner may administer policy; protected operations still require explicit grants"
            );
            0
        }
        Err(error) => {
            eprintln!("made-mcp: bootstrap-authorization: {error}");
            eprintln!("made-mcp: the authorization policy was not changed");
            1
        }
    }
}

#[cfg(not(feature = "embedded"))]
#[allow(clippy::unused_async)]
async fn run_bootstrap_authorization(_path: &str, _policy_id: &str, _trusted_host_id: &str) -> i32 {
    eprintln!("made-mcp: bootstrap-authorization needs the embedded engine");
    2
}

/// Bring the sessions of a pre-stream store into their own streams
/// (ADR-012), copy-on-write.
#[cfg(feature = "embedded")]
async fn run_migrate_store(path: &str) -> i32 {
    let path = std::path::Path::new(path);
    match made_mcp::migrate_store(path).await {
        Ok(outcome) => {
            for line in made_mcp::migrate_store_report(path, &outcome) {
                println!("{line}");
            }
            0
        }
        Err(message) => {
            eprintln!("made-mcp: migrate-store: {message}");
            eprintln!("made-mcp: the store was not changed");
            1
        }
    }
}

/// Without the embedded engine there is no store to migrate: this
/// binary speaks to a service, and a service's store is migrated where
/// it lives.
#[cfg(not(feature = "embedded"))]
#[allow(clippy::unused_async)]
async fn run_migrate_store(_path: &str) -> i32 {
    eprintln!(
        "made-mcp: migrate-store needs the embedded engine; this binary was built without it"
    );
    2
}

#[cfg(feature = "embedded")]
fn embedded_store_carried() -> &'static str {
    "embedded store: sqlite"
}

#[cfg(not(feature = "embedded"))]
fn embedded_store_carried() -> &'static str {
    "no embedded store"
}
