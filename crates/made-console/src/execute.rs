use std::path::Path;
use std::time::Duration;

use made_client::v1::{
    ApproveCeremonyGuardRequest, CancelCeremonyRequest, PauseCeremonyRequest,
    ResumeCeremonyRequest, StreamCeremonyEndReason,
};
use made_client::{ClientConfig, MadeClient, MadeClientError, ProgressCheckpoint};
use serde_json::json;

use crate::{render, Args, ArtifactCommand, BudgetCommand, Command, OutputFormat};

pub async fn run(args: Args) -> Result<(), MadeClientError> {
    let client = connect_client(&args).await?;
    match args.command {
        Command::Get { ceremony_id } => println!(
            "{}",
            render::instance(&client.get_ceremony(ceremony_id).await?, args.output)
        ),
        Command::List => println!(
            "{}",
            render::instances(&client.list_ceremonies().await?, args.output)
        ),
        Command::Tree {
            ceremony_id,
            max_nodes,
        } => println!(
            "{}",
            render::tree(
                &client
                    .ceremony_tree(ceremony_id, max_nodes as usize)
                    .await?,
                args.output
            )
        ),
        Command::Watch {
            ceremony_id,
            cursor_file,
            after_sequence,
            limit,
            wait_ms,
            follow,
        } => {
            watch(
                &client,
                &ceremony_id,
                cursor_file.as_deref(),
                after_sequence,
                limit,
                wait_ms,
                follow,
                args.output,
            )
            .await?;
        }
        Command::Artifact { command } => artifact(&client, command, args.output).await?,
        Command::Budget { command } => budget(&client, command, args.output).await?,
        Command::Report {
            ceremony_ids,
            title,
            destination,
        } => report(&client, ceremony_ids, title, &destination, args.output).await?,
        command => action(&client, command, args.output).await?,
    }
    Ok(())
}

async fn connect_client(args: &Args) -> Result<MadeClient, MadeClientError> {
    let mut config = ClientConfig::new(args.endpoint.clone());
    if let Some(request_id) = &args.request_id {
        config = config.with_request_id(request_id.clone());
    }
    if let Some(path) = &args.tls_ca_certificate {
        config = config.with_ca_certificate_pem(read_pem(path).await?);
    }
    if let (Some(certificate), Some(key)) = (&args.tls_client_certificate, &args.tls_client_key) {
        config = config.with_mtls_identity_pem(read_pem(certificate).await?, read_pem(key).await?);
    }
    if let Some(domain_name) = &args.tls_domain_name {
        config = config.with_tls_domain_name(domain_name.clone());
    }
    MadeClient::connect_with_config(config).await
}

async fn read_pem(path: &Path) -> Result<Vec<u8>, MadeClientError> {
    tokio::fs::read(path)
        .await
        .map_err(|error| MadeClientError::Io {
            path: path.display().to_string(),
            source: error,
        })
}

async fn budget(
    client: &MadeClient,
    command: BudgetCommand,
    output: OutputFormat,
) -> Result<(), MadeClientError> {
    match command {
        BudgetCommand::Report { ceremony_id } => {
            let report = client.get_budget_report(ceremony_id).await?;
            println!("{}", render::budget_report(&report, output));
        }
        BudgetCommand::Pending { after, limit } => {
            let page = client
                .list_pending_budget_reservations(after, limit)
                .await?;
            println!("{}", render::pending_budget_reservations(&page, output));
        }
    }
    Ok(())
}

async fn watch(
    client: &MadeClient,
    ceremony_id: &str,
    cursor_file: Option<&Path>,
    after_sequence: u64,
    limit: u32,
    wait_ms: u32,
    follow: bool,
    output: OutputFormat,
) -> Result<(), MadeClientError> {
    let mut checkpoint = load_checkpoint(cursor_file, ceremony_id, after_sequence).await?;
    loop {
        match client.watch_once(&checkpoint, limit, Some(wait_ms)).await {
            Ok(batch) => {
                println!("{}", render::progress(&batch, output));
                let terminal_caught_up = batch.end_reason() == StreamCeremonyEndReason::Terminal
                    && batch.checkpoint().after_sequence() >= batch.head_sequence();
                checkpoint = batch.checkpoint().clone();
                if let Some(path) = cursor_file {
                    checkpoint.save(path).await?;
                }
                if !follow || terminal_caught_up {
                    return Ok(());
                }
            }
            Err(error) if follow && error.is_retryable_read() => {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn load_checkpoint(
    cursor_file: Option<&Path>,
    ceremony_id: &str,
    after_sequence: u64,
) -> Result<ProgressCheckpoint, MadeClientError> {
    let Some(path) = cursor_file else {
        return Ok(ProgressCheckpoint::new(ceremony_id, after_sequence));
    };
    let exists = tokio::fs::try_exists(path)
        .await
        .map_err(|error| MadeClientError::Io {
            path: path.display().to_string(),
            source: error,
        })?;
    if exists {
        ProgressCheckpoint::load(path, ceremony_id).await
    } else {
        Ok(ProgressCheckpoint::new(ceremony_id, after_sequence))
    }
}

async fn artifact(
    client: &MadeClient,
    command: ArtifactCommand,
    output: OutputFormat,
) -> Result<(), MadeClientError> {
    match command {
        ArtifactCommand::List { cursor, limit } => {
            let page = client.list_artifacts(cursor, limit).await?;
            println!(
                "{}",
                render::artifacts(&page.artifacts, page.next_cursor.as_deref(), output)
            );
        }
        ArtifactCommand::Export {
            artifact_id,
            destination,
            force,
        } => {
            client
                .export_artifact(&artifact_id, &destination, force)
                .await?;
            println!(
                "{}",
                render::message(
                    &json!({ "artifact_id": artifact_id, "destination": destination }),
                    output,
                )
            );
        }
    }
    Ok(())
}

async fn report(
    client: &MadeClient,
    ceremony_ids: Vec<String>,
    title: String,
    destination: &Path,
    output: OutputFormat,
) -> Result<(), MadeClientError> {
    let response = client
        .export_report(ceremony_ids, title, destination)
        .await?;
    println!(
        "{}",
        render::message(
            &json!({
                "destination": destination,
                "ceremony_count": response.ceremony_count,
                "completed_count": response.completed_count,
                "incomplete_count": response.incomplete_count,
            }),
            output,
        )
    );
    Ok(())
}

async fn action(
    client: &MadeClient,
    command: Command,
    output: OutputFormat,
) -> Result<(), MadeClientError> {
    let instance = match command {
        Command::Pause {
            ceremony_id,
            actor_id,
            actor_kind,
            reason,
        } => {
            client
                .pause(PauseCeremonyRequest {
                    ceremony_id,
                    actor_id,
                    actor_kind,
                    reason,
                })
                .await?
        }
        Command::Resume {
            ceremony_id,
            actor_id,
            actor_kind,
        } => {
            client
                .resume(ResumeCeremonyRequest {
                    ceremony_id,
                    actor_id,
                    actor_kind,
                })
                .await?
        }
        Command::Cancel {
            ceremony_id,
            actor_id,
            actor_kind,
            reason,
        } => {
            client
                .cancel(CancelCeremonyRequest {
                    ceremony_id,
                    actor_id,
                    actor_kind,
                    reason,
                })
                .await?
        }
        Command::EnforceDeadlines { ceremony_id } => client.enforce_deadlines(ceremony_id).await?,
        Command::Approve {
            ceremony_id,
            guard,
            role_id,
            role_kind,
        } => {
            client
                .approve(ApproveCeremonyGuardRequest {
                    ceremony_id,
                    guard_name: guard,
                    role_id,
                    role_kind,
                })
                .await?
        }
        _ => {
            return Err(MadeClientError::ProtocolViolation(
                "non-action command reached action dispatcher".to_owned(),
            ));
        }
    };
    println!("{}", render::instance(&instance, output));
    Ok(())
}
