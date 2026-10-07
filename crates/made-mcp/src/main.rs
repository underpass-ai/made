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

    if server.tool_profile().name() != made_mcp::tool_profile::FULL_PROFILE_NAME {
        eprintln!(
            "made-mcp: listing tool profile `{}` from {}; hidden tools are refused by name, \
             restart with a wider profile to list them",
            server.tool_profile().name(),
            made_mcp::TOOL_PROFILE_ENV
        );
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
        "approve-guard" => run_approve_guard(args).await,
        "grant" => run_grant(args).await,
        "keygen" => run_keygen(args),
        "public-key" => run_public_key(args),
        "export-evidence" => run_export_evidence(args).await,
        "verify-evidence" => run_verify_evidence(args),
        other => {
            eprintln!(
                "made-mcp: unknown command `{other}`; run without arguments for MCP stdio mode, or use `--version`, `migrate-store <path>`, `bootstrap-authorization <store> --policy-id <id> --trusted-host-id <id>`, `{APPROVE_GUARD_USAGE}`, `{}`, `{KEYGEN_USAGE}`, `{EXPORT_EVIDENCE_USAGE}` or `{VERIFY_EVIDENCE_USAGE}`",
                made_mcp::GRANT_COMMAND.trim_start_matches("made-mcp ")
            );
            2
        }
    }
}

const APPROVE_GUARD_USAGE: &str =
    "approve-guard <store> --ceremony <id> --guard <name> --role <role> [--reason <text>]";

/// A person approves a human guard from their own terminal. The policy
/// and trusted host come from the same environment the launcher gives
/// the server, so the approval is admitted by the same policy.
#[cfg(feature = "embedded")]
async fn run_approve_guard(args: &[String]) -> i32 {
    use made_mcp::approve_guard_command::{
        ApproveGuardCommand, ApproveGuardOutcome, StdioTerminal,
    };

    let Some((store, flags)) = args.split_first() else {
        eprintln!("made-mcp: usage: made-mcp {APPROVE_GUARD_USAGE}");
        return 2;
    };
    let mut ceremony = None;
    let mut guard = None;
    let mut role = None;
    let mut reason = None;
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        let slot = match flag.as_str() {
            "--ceremony" => &mut ceremony,
            "--guard" => &mut guard,
            "--role" => &mut role,
            "--reason" => &mut reason,
            other => {
                eprintln!("made-mcp: approve-guard: unknown flag `{other}`");
                eprintln!("made-mcp: usage: made-mcp {APPROVE_GUARD_USAGE}");
                return 2;
            }
        };
        let Some(value) = flags.next() else {
            eprintln!("made-mcp: approve-guard: `{flag}` needs a value");
            return 2;
        };
        *slot = Some(value.clone());
    }
    let (Some(ceremony), Some(guard), Some(role)) = (ceremony, guard, role) else {
        eprintln!("made-mcp: usage: made-mcp {APPROVE_GUARD_USAGE}");
        return 2;
    };
    let Some((policy_id, trusted_host_id)) =
        terminal_policy_identity("approve-guard", "scripts/made-approve.sh")
    else {
        return 2;
    };
    let command = match ApproveGuardCommand::new(
        store,
        &policy_id,
        &trusted_host_id,
        &ceremony,
        &guard,
        &role,
        reason,
    ) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("made-mcp: approve-guard: {error}");
            return 2;
        }
    };
    match command.run(&mut StdioTerminal).await {
        Ok(ApproveGuardOutcome::Recorded { .. }) => 0,
        Ok(ApproveGuardOutcome::Declined) => 3,
        Err(error) => {
            eprintln!("made-mcp: approve-guard: {error}");
            1
        }
    }
}

/// The policy and trusted host a terminal command acts as: the same
/// two values the launcher gives the server, so what the terminal seals
/// is admitted by the same policy. The plugin's wrapper scripts set
/// them from setup; by hand they are exported before the command.
#[cfg(feature = "embedded")]
fn terminal_policy_identity(command: &str, script: &str) -> Option<(String, String)> {
    let read = |name: &str| match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!(
                "made-mcp: {command}: {name} is required (the plugin's {script} sets it from setup)"
            );
            None
        }
    };
    let policy_id = read("MADE_AUTH_POLICY_ID")?;
    let trusted_host_id = read("MADE_AUTH_TRUSTED_HOST_ID")?;
    Some((policy_id, trusted_host_id))
}

/// A person decides what the host may do, from their own terminal.
#[cfg(feature = "embedded")]
async fn run_grant(args: &[String]) -> i32 {
    use made_mcp::grant_command::{GrantArguments, GrantCommand, GrantOutcome, NOT_INTERACTIVE};
    use made_mcp::terminal::StdioTerminal;

    let arguments = match GrantArguments::parse(args) {
        Ok(arguments) => arguments,
        Err(error) => {
            eprintln!("made-mcp: grant: {error}");
            eprintln!("made-mcp: usage: {}", made_mcp::GRANT_COMMAND);
            return 2;
        }
    };
    let Some((policy_id, trusted_host_id)) =
        terminal_policy_identity("grant", made_mcp::GRANT_SCRIPT)
    else {
        return 2;
    };
    if arguments.show {
        return match GrantCommand::show_authority(&arguments, &policy_id, &trusted_host_id).await {
            Ok(lines) => {
                println!(
                    "made-mcp: grant: authority details computed ({} line(s)); detailed output suppressed",
                    lines.len()
                );
                0
            }
            Err(error) => {
                eprintln!("made-mcp: grant: {error}");
                1
            }
        };
    }
    let command = match GrantCommand::from_arguments(&arguments, &policy_id, &trusted_host_id) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("made-mcp: grant: {error}");
            eprintln!("made-mcp: usage: {}", made_mcp::GRANT_COMMAND);
            return 2;
        }
    };
    match command.run(&mut StdioTerminal).await {
        Ok(GrantOutcome::Recorded { .. }) => 0,
        Ok(GrantOutcome::Declined) => 3,
        Err(error) => {
            eprintln!("made-mcp: grant: {error}");
            i32::from(error != NOT_INTERACTIVE) + 1
        }
    }
}

#[cfg(not(feature = "embedded"))]
#[allow(clippy::unused_async)]
async fn run_grant(_args: &[String]) -> i32 {
    eprintln!("made-mcp: grant needs the embedded engine; this binary was built without it");
    2
}

#[cfg(not(feature = "embedded"))]
#[allow(clippy::unused_async)]
async fn run_approve_guard(_args: &[String]) -> i32 {
    eprintln!(
        "made-mcp: approve-guard needs the embedded engine; this binary was built without it"
    );
    2
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

const KEYGEN_USAGE: &str = "keygen <key-file>";
const EXPORT_EVIDENCE_USAGE: &str =
    "export-evidence <store> --ceremony <id> --key <key-file> --out <file>";
const VERIFY_EVIDENCE_USAGE: &str = "verify-evidence <file> [--public-key <hex>]";

/// Read `--flag value` pairs from `args` into the named slots; an
/// unknown flag or a flag without a value is a usage error. Only the
/// embedded arms of the evidence commands parse flags.
#[cfg(feature = "embedded")]
fn read_flags<'a>(
    args: &'a [String],
    slots: &mut [(&str, &mut Option<&'a str>)],
) -> Result<(), String> {
    let mut remaining = args.iter();
    while let Some(flag) = remaining.next() {
        let Some(slot) = slots.iter_mut().find(|(name, _)| name == flag) else {
            return Err(format!("unknown flag `{flag}`"));
        };
        let Some(value) = remaining.next() else {
            return Err(format!("`{flag}` needs a value"));
        };
        *slot.1 = Some(value.as_str());
    }
    Ok(())
}

#[cfg(feature = "embedded")]
fn run_keygen(args: &[String]) -> i32 {
    let [path] = args else {
        eprintln!("made-mcp: usage: made-mcp {KEYGEN_USAGE}");
        return 2;
    };
    match made_mcp::evidence_command::keygen(std::path::Path::new(path)) {
        Ok(public_key) => {
            println!("evidence signing key written to {path} (owner-readable only)");
            println!("public key: {public_key}");
            println!("hand the public key to whoever will verify your exports; never the file");
            0
        }
        Err(error) => {
            eprintln!("made-mcp: keygen: {error}");
            1
        }
    }
}

#[cfg(feature = "embedded")]
fn run_public_key(args: &[String]) -> i32 {
    let [path] = args else {
        eprintln!("made-mcp: usage: made-mcp public-key <key-file>");
        return 2;
    };
    match made_mcp::evidence_command::public_key(std::path::Path::new(path)) {
        Ok(public_key) => {
            println!("{public_key}");
            0
        }
        Err(error) => {
            eprintln!("made-mcp: public-key: {error}");
            1
        }
    }
}

#[cfg(feature = "embedded")]
async fn run_export_evidence(args: &[String]) -> i32 {
    let Some((store, flags)) = args.split_first() else {
        eprintln!("made-mcp: usage: made-mcp {EXPORT_EVIDENCE_USAGE}");
        return 2;
    };
    let (mut ceremony, mut key, mut out) = (None, None, None);
    if let Err(error) = read_flags(
        flags,
        &mut [
            ("--ceremony", &mut ceremony),
            ("--key", &mut key),
            ("--out", &mut out),
        ],
    ) {
        eprintln!("made-mcp: export-evidence: {error}");
        eprintln!("made-mcp: usage: made-mcp {EXPORT_EVIDENCE_USAGE}");
        return 2;
    }
    let (Some(ceremony), Some(key), Some(out)) = (ceremony, key, out) else {
        eprintln!("made-mcp: usage: made-mcp {EXPORT_EVIDENCE_USAGE}");
        return 2;
    };
    match made_mcp::evidence_command::export(
        std::path::Path::new(store),
        ceremony,
        std::path::Path::new(key),
        std::path::Path::new(out),
    )
    .await
    {
        Ok(receipt) => {
            for line in receipt.lines() {
                println!("{line}");
            }
            0
        }
        Err(error) => {
            eprintln!("made-mcp: export-evidence: {error}");
            1
        }
    }
}

#[cfg(feature = "embedded")]
fn run_verify_evidence(args: &[String]) -> i32 {
    let Some((file, flags)) = args.split_first() else {
        eprintln!("made-mcp: usage: made-mcp {VERIFY_EVIDENCE_USAGE}");
        return 2;
    };
    let mut public_key = None;
    if let Err(error) = read_flags(flags, &mut [("--public-key", &mut public_key)]) {
        eprintln!("made-mcp: verify-evidence: {error}");
        eprintln!("made-mcp: usage: made-mcp {VERIFY_EVIDENCE_USAGE}");
        return 2;
    }
    match made_mcp::evidence_command::verify(std::path::Path::new(file), public_key) {
        Ok(verification) => {
            for line in verification.lines() {
                println!("{line}");
            }
            i32::from(!verification.is_sound())
        }
        Err(error) => {
            eprintln!("made-mcp: verify-evidence: {error}");
            2
        }
    }
}

#[cfg(not(feature = "embedded"))]
fn run_keygen(_args: &[String]) -> i32 {
    eprintln!("made-mcp: keygen needs the embedded engine; this binary was built without it");
    2
}

#[cfg(not(feature = "embedded"))]
fn run_public_key(_args: &[String]) -> i32 {
    eprintln!("made-mcp: public-key needs the embedded engine; this binary was built without it");
    2
}

#[cfg(not(feature = "embedded"))]
#[allow(clippy::unused_async)]
async fn run_export_evidence(_args: &[String]) -> i32 {
    eprintln!(
        "made-mcp: export-evidence needs the embedded engine; this binary was built without it"
    );
    2
}

#[cfg(not(feature = "embedded"))]
fn run_verify_evidence(_args: &[String]) -> i32 {
    eprintln!(
        "made-mcp: verify-evidence needs the embedded engine; this binary was built without it"
    );
    2
}
