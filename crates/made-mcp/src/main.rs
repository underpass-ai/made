//! `made-mcp` — stdio MCP adapter for MADE.
//!
//! Reads one JSON-RPC line at a time from stdin, dispatches to the
//! inner [`MadeMcpServer`], writes the response to stdout. Stdout
//! is reserved for JSON-RPC responses; logs go to stderr as JSON.
//!
//! See `docs/operations/mcp-stdio.md` for end-user setup.

use std::io::{self, BufRead, Write};

use made_mcp::{
    MadeMcpServer, EMBEDDED_REDB_PATH_ENV, GRPC_ENDPOINT_ENV, GRPC_TLS_CA_PATH_ENV,
    GRPC_TLS_CERT_PATH_ENV, GRPC_TLS_DOMAIN_NAME_ENV, GRPC_TLS_KEY_PATH_ENV, GRPC_TLS_MODE_ENV,
    LEGACY_REDB_PATH_ENV, MCP_BACKEND_ENV,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    // One maintenance command, and only because the feature is unusable
    // without it: a store that already exists cannot reach the engine that
    // lets two hosts share it. Everything else this binary does is MCP over
    // stdio, and stays that way.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(command) = args.first() {
        let code = run_cli_command(command, &args[1..]);
        std::process::exit(code);
    }

    let server = match MadeMcpServer::try_from_env() {
        Ok(server) => server,
        Err(message) => {
            eprintln!("made-mcp: {message}");
            // Only a backend-selection failure is fixed by choosing a
            // backend. Printing this after a store lock or a bad path sent
            // operators to look exactly where the problem was not.
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
            "made-mcp: using durable embedded ceremony backend from {EMBEDDED_REDB_PATH_ENV}"
        );
        if std::env::var_os(LEGACY_REDB_PATH_ENV).is_some() {
            eprintln!("made-mcp: legacy import configured from read-only {LEGACY_REDB_PATH_ENV}");
        }
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
        .unwrap_or_else(|_| EnvFilter::new("made_mcp=info,made_adapters::redb=info"));
    tracing_subscriber::fmt()
        .json()
        .with_writer(io::stderr)
        .with_env_filter(filter)
        .init();
}

/// `convert <source> <destination> --engine redb|sqlite`.
///
/// Converting rather than migrating: a ceremony store is state plus an audit
/// journal, so the copy moves rows, and replaying the journal would rebuild
/// the facts while losing what they are evidence of.
fn run_cli_command(command: &str, args: &[String]) -> i32 {
    match command {
        #[cfg(feature = "embedded")]
        "convert" => run_convert_command(args),
        "--version" | "-V" | "version" => {
            println!("made-mcp {}", env!("CARGO_PKG_VERSION"));
            0
        }
        other => {
            eprintln!(
                "made-mcp: unknown command `{other}`; run without arguments for MCP stdio \
                 mode, or use `convert <source> <destination> --engine redb|sqlite`"
            );
            2
        }
    }
}

#[cfg(feature = "embedded")]
fn run_convert_command(args: &[String]) -> i32 {
    use made_adapters::redb::RedbCeremonyStore;
    use made_adapters::StorageEngine;

    let [source, destination, rest @ ..] = args else {
        eprintln!("made-mcp: convert requires <source> <destination> --engine redb|sqlite");
        return 2;
    };
    let engine = match rest {
        [flag, value] if flag == "--engine" => {
            let Ok(engine) = StorageEngine::parse(value) else {
                eprintln!("made-mcp: unknown engine `{value}`; expected `redb` or `sqlite`");
                return 2;
            };
            engine
        }
        [] => {
            eprintln!("made-mcp: convert needs --engine redb|sqlite: it is the whole point");
            return 2;
        }
        _ => {
            eprintln!("made-mcp: convert takes <source> <destination> --engine redb|sqlite");
            return 2;
        }
    };

    match RedbCeremonyStore::convert(source, destination, engine) {
        Ok(receipt) => {
            println!(
                "{{\"source_engine\":\"{}\",\"destination_engine\":\"{}\",\
                 \"ceremonies\":{},\"journal_records\":{},\"outbox_messages\":{},\
                 \"publications\":{}}}",
                receipt.source_engine,
                receipt.destination_engine,
                receipt.ceremonies,
                receipt.journal_records,
                receipt.outbox_messages,
                receipt.publications
            );
            0
        }
        Err(error) => {
            eprintln!("made-mcp: conversion failed: {error}");
            2
        }
    }
}
