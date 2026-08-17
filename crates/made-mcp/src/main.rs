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
        let code = run_cli_command(command, &args[1..]).await;
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
// `share-store` awaits the store it verifies, and that arm only exists in an
// embedded build — so without it this function has nothing to await. It stays
// async either way rather than forking the dispatch on a cargo feature.
#[cfg_attr(not(feature = "embedded"), allow(clippy::unused_async))]
async fn run_cli_command(command: &str, args: &[String]) -> i32 {
    match command {
        #[cfg(feature = "embedded")]
        "convert" => run_convert_command(args),
        #[cfg(feature = "embedded")]
        "share-store" => run_share_store_command(args).await,
        // The command exists; this build has no store for it to act on.
        // "unknown command" would send an operator looking for a typo.
        #[cfg(not(feature = "embedded"))]
        "convert" => {
            let _ = args;
            eprintln!(
                "made-mcp: `convert` moves an embedded store between engines, and this \
                 binary was built without the embedded backend"
            );
            2
        }
        "--version" | "-V" | "version" => {
            // Which engines this build can open. Whether the binary carries
            // sqlite is the most consequential thing about a MADE install —
            // it decides whether two hosts can share a store — and it used to
            // take a failed store open to find out.
            println!(
                "made-mcp {} ({})",
                env!("CARGO_PKG_VERSION"),
                engines_carried()
            );
            0
        }
        other => {
            eprintln!(
                "made-mcp: unknown command `{other}`; run without arguments for MCP stdio \
                 mode, or use `share-store [path]` / \
                 `convert <source> <destination> --engine redb|sqlite`"
            );
            2
        }
    }
}

/// The engines this build can open, for `--version`.
///
/// Whether the binary carries sqlite is the most consequential thing about a
/// MADE install — it decides whether two hosts can share a store — and it
/// used to take a failed store open to find out.
#[cfg(feature = "embedded")]
fn engines_carried() -> String {
    use made_adapters::StorageEngine;
    let carried: Vec<&str> = [StorageEngine::Redb, StorageEngine::Sqlite]
        .into_iter()
        .filter(|engine| engine.is_compiled())
        .map(StorageEngine::name)
        .collect();
    format!("engines: {}", carried.join(", "))
}

#[cfg(not(feature = "embedded"))]
fn engines_carried() -> String {
    "no embedded engine".to_string()
}

/// The store this run should convert, or `None` when there is nothing to do.
///
/// A store already on sqlite is a no-op that says so, rather than a second
/// conversion: running this twice must be safe.
#[cfg(feature = "embedded")]
fn share_store_target(args: &[String]) -> Result<Option<std::path::PathBuf>, i32> {
    use made_adapters::{engine_of, StorageEngine};
    use std::path::PathBuf;

    let store_path: PathBuf = match args {
        [] => {
            let Ok(path) = std::env::var("MADE_MCP_REDB_PATH") else {
                eprintln!(
                    "made-mcp: share-store needs the store path, either as an argument or in \
                     MADE_MCP_REDB_PATH"
                );
                return Err(2);
            };
            PathBuf::from(path)
        }
        [path] => PathBuf::from(path),
        _ => {
            eprintln!("made-mcp: share-store takes at most one path");
            return Err(2);
        }
    };

    match engine_of(&store_path) {
        Ok(Some(StorageEngine::Sqlite)) => {
            println!(
                "already shareable: `{}` is on the sqlite engine. Point both hosts at it.",
                store_path.display()
            );
            Ok(None)
        }
        Ok(Some(StorageEngine::Redb)) => Ok(Some(store_path)),
        Ok(None) => {
            eprintln!(
                "made-mcp: no ceremony store at `{}` yet. Start one with \
                 MADE_MCP_ENGINE=sqlite and it is shareable from the first write.",
                store_path.display()
            );
            Err(2)
        }
        Err(error) => {
            eprintln!("made-mcp: cannot read `{}`: {error}", store_path.display());
            Err(2)
        }
    }
}

/// `share-store [path]` — convert a ceremony store in place, safely.
///
/// `convert` is the mechanism: two paths, one engine, a receipt. Using it to
/// actually move a live store meant knowing that the store is locked by the
/// session asking (so it has to be snapshotted first), that nothing checks
/// the result holds what the source held, and that getting the swap order
/// wrong leaves two live stores or none. Three things to know, none of them
/// written down where an operator would look.
///
/// Nothing is deleted: the original is kept beside the new one.
#[cfg(feature = "embedded")]
async fn run_share_store_command(args: &[String]) -> i32 {
    use made_adapters::redb::RedbCeremonyStore;
    use made_adapters::StorageEngine;
    use std::path::PathBuf;

    if !StorageEngine::Sqlite.is_compiled() {
        eprintln!(
            "made-mcp: this binary was built without the sqlite engine, so it cannot share a \
             store between hosts.\n  install one with: cargo install made-mcp --features sqlite\n               (then re-run this command; nothing has been changed)"
        );
        return 2;
    }

    let store_path: PathBuf = match share_store_target(args) {
        Ok(Some(path)) => path,
        Ok(None) => return 0,
        Err(code) => return code,
    };

    // The live store is very likely held by the session asking for this, and
    // redb is single-writer — so the conversion reads a snapshot, never the
    // original. Copying a file is a read; it does not need the lock.
    let snapshot = sibling(&store_path, "share-store-snapshot");
    // The converted store takes the name its engine implies. A SQLite store
    // living at `ceremonies.redb` works — the engine is read from the first
    // bytes and never from the name — but anyone who lists that directory
    // will conclude the conversion failed.
    let installed = store_path.with_extension("sqlite3");
    let converted = sibling(&store_path, "share-store-converted");
    if installed.exists() && installed != store_path {
        eprintln!(
            "made-mcp: `{}` already exists, so this would leave two stores. Nothing was changed.",
            installed.display()
        );
        return 2;
    }
    for path in [&snapshot, &converted] {
        if path.exists() {
            eprintln!(
                "made-mcp: `{}` is left over from an earlier run; move or remove it first. \
                 Nothing has been changed.",
                path.display()
            );
            return 2;
        }
    }
    if let Err(error) = std::fs::copy(&store_path, &snapshot) {
        eprintln!(
            "made-mcp: could not snapshot `{}`: {error}",
            store_path.display()
        );
        return 2;
    }
    println!("snapshot taken (the live store was not touched)");

    let receipt = match RedbCeremonyStore::convert(&snapshot, &converted, StorageEngine::Sqlite) {
        Ok(receipt) => receipt,
        Err(error) => {
            let _ = std::fs::remove_file(&snapshot);
            eprintln!("made-mcp: conversion failed, nothing was changed: {error}");
            return 2;
        }
    };
    println!(
        "converted: {} ceremonies, {} journal records, {} publications",
        receipt.ceremonies, receipt.journal_records, receipt.publications
    );

    // Verify before swapping, not after. A conversion that reports success
    // and drops ceremonies would otherwise be found by an operator, later.
    match verify_same_ceremonies(&snapshot, &converted).await {
        Ok(count) => println!("verified: {count} ceremonies on both engines"),
        Err(error) => {
            let _ = std::fs::remove_file(&snapshot);
            let _ = std::fs::remove_file(&converted);
            eprintln!(
                "made-mcp: the converted store does not match the original, so nothing was \
                 changed: {error}"
            );
            return 2;
        }
    }

    let kept = sibling(&store_path, "redb-before-share");
    if let Err(code) = install_converted_store(&store_path, &converted, &kept) {
        return code;
    }
    let _ = std::fs::remove_file(&snapshot);

    println!(
        "\n`{}` is now on the sqlite engine and two hosts can share it.\n\
         the original is kept at `{}` — nothing was deleted",
        installed.display(),
        kept.display()
    );
    // The launcher finds the sqlite store on its own once the redb one is
    // out of the way. An operator who names the path themselves has to move
    // their pointer, and would otherwise find out by losing their history.
    if std::env::var("MADE_MCP_REDB_PATH")
        .is_ok_and(|path| std::path::Path::new(&path) == store_path)
    {
        println!(
            "\nMADE_MCP_REDB_PATH still points at the old name — set it to `{}`",
            installed.display()
        );
    }
    println!("restart every agent host so it opens the new store");
    0
}

/// Moves the original aside and puts the converted store in its place.
///
/// In that order, and never the other way: a failure between the two leaves
/// the original where the error message says it is, rather than leaving two
/// live stores or none.
#[cfg(feature = "embedded")]
fn install_converted_store(
    store_path: &std::path::Path,
    converted: &std::path::Path,
    kept: &std::path::Path,
) -> Result<(), i32> {
    if kept.exists() {
        eprintln!(
            "made-mcp: `{}` already exists, so the original cannot be moved aside safely; \
             nothing was changed",
            kept.display()
        );
        return Err(2);
    }
    if let Err(error) = std::fs::rename(store_path, kept) {
        eprintln!("made-mcp: could not move the original aside: {error}");
        return Err(2);
    }
    let installed = store_path.with_extension("sqlite3");
    if let Err(error) = std::fs::rename(converted, &installed) {
        eprintln!(
            "made-mcp: could not install the converted store; the original is intact at `{}`: \
             {error}",
            kept.display()
        );
        return Err(2);
    }
    Ok(())
}

/// A working file beside the store, so a rename lands on the same filesystem.
#[cfg(feature = "embedded")]
fn sibling(store: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let name = store.file_name().map_or_else(
        || "ceremonies".to_string(),
        |name| name.to_string_lossy().to_string(),
    );
    store.with_file_name(format!("{name}.{suffix}"))
}

/// Both stores must hold the same ceremonies.
#[cfg(feature = "embedded")]
async fn verify_same_ceremonies(
    original: &std::path::Path,
    converted: &std::path::Path,
) -> Result<usize, String> {
    async fn count(path: &std::path::Path) -> Result<usize, String> {
        use made_adapters::redb::RedbCeremonyStore;
        use made_core::ports::CeremonyInstanceRepositoryPort;

        let store = RedbCeremonyStore::open(path)
            .map_err(|error| format!("could not open `{}`: {error}", path.display()))?;
        store
            .list()
            .await
            .map(|instances| instances.len())
            .map_err(|error| format!("could not list `{}`: {error}", path.display()))
    }
    let before = count(original).await?;
    let after = count(converted).await?;
    if before != after {
        return Err(format!(
            "original holds {before} ceremonies, converted holds {after}"
        ));
    }
    Ok(after)
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
