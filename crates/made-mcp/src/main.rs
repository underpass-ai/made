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
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(command) = args.first() {
        std::process::exit(run_cli_command(command, &args[1..]));
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

    let stdin = io::stdin();
    let mut stdout = io::stdout();

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

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("made_mcp=info,made_adapters::sqlite=info"));
    tracing_subscriber::fmt()
        .json()
        .with_writer(io::stderr)
        .with_env_filter(filter)
        .init();
}

fn run_cli_command(command: &str, args: &[String]) -> i32 {
    match command {
        "--version" | "-V" | "version" if args.is_empty() => {
            println!(
                "made-mcp {} ({})",
                env!("CARGO_PKG_VERSION"),
                embedded_store_carried()
            );
            0
        }
        other => {
            eprintln!(
                "made-mcp: unknown command `{other}`; run without arguments for MCP stdio mode, or use `--version`"
            );
            2
        }
    }
}

#[cfg(feature = "embedded")]
fn embedded_store_carried() -> &'static str {
    "embedded store: sqlite"
}

#[cfg(not(feature = "embedded"))]
fn embedded_store_carried() -> &'static str {
    "no embedded store"
}
