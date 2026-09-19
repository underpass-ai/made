use std::path::Path;
use std::time::Duration;

use made_client::v1::{
    ApproveCeremonyGuardRequest, CancelCeremonyRequest, PauseCeremonyRequest,
    ResumeCeremonyRequest, StreamCeremonyEndReason,
};
use made_client::{MadeClient, MadeClientError, ProgressCheckpoint};
use serde_json::json;

use crate::{render, Args, ArtifactCommand, Command, OutputFormat};

pub async fn run(args: Args) -> Result<(), MadeClientError> {
    let client = MadeClient::connect(args.endpoint).await?;
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
        Command::Report {
            ceremony_ids,
            title,
            destination,
        } => report(&client, ceremony_ids, title, &destination, args.output).await?,
        command => action(&client, command, args.output).await?,
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
