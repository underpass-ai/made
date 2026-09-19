use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use futures::StreamExt;
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::stream_ceremony_response::Frame;
use made_proto::v1::{
    ApplyCeremonyTransitionRequest, CeremonyEventRecord, GenerateCeremonyReportRequest,
    GetCeremonyInstanceRequest, PublishCeremonyDefinitionRequest, ReadCeremonyEventsRequest,
    RunCeremonyStepRequest, StartPublishedCeremonyRequest, StreamCeremonyEnd,
    StreamCeremonyEndReason, StreamCeremonyRequest, VerifyCeremonyJournalRequest,
};
use prost_types::{value::Kind, Struct, Value};
use tonic::transport::Channel;
use tracing::info;

use super::children_ceremony_definitions::ChildrenCeremonyDefinitions;

const PROGRESS_ID: &str = "e2e-ceremony-progress";

pub(crate) async fn verify_live_ceremony_progress(
    client: &mut MadeServiceClient<Channel>,
) -> Result<()> {
    publish(client, ChildrenCeremonyDefinitions::child()).await?;
    client
        .start_published_ceremony(StartPublishedCeremonyRequest {
            ceremony_id: PROGRESS_ID.to_owned(),
            ceremony: "children_review".to_owned(),
            version: "1.0".to_owned(),
            context: Some(string_context(
                "review_brief",
                "Exercise the public ceremony progress stream.",
            )),
            actor_id: "progress-e2e-operator".to_owned(),
            actor_kind: "service".to_owned(),
            budget_limits: None,
        })
        .await
        .context("StartPublishedCeremony failed for progress scenario")?;

    let initial = read_events(client).await?;
    let initial_head = initial.len() as u64;
    assert_idle_at_head(client, initial_head).await?;
    let live_records = follow_to_terminal(client, initial_head).await?;

    let all_records = read_events(client).await?;
    let terminal = assert_resumable_replay(client, &all_records).await?;
    assert_terminal_journal(client, terminal.head_sequence, all_records.len()).await?;
    assert_report(client).await?;

    info!(
        ceremony_id = PROGRESS_ID,
        live_records = live_records.len(),
        journal_records = all_records.len(),
        "live progress, bounded replay, resume, and journal agree"
    );
    Ok(())
}

async fn assert_idle_at_head(client: &mut MadeServiceClient<Channel>, head: u64) -> Result<()> {
    let (records, end) = stream(
        client,
        StreamCeremonyRequest {
            ceremony_id: PROGRESS_ID.to_owned(),
            after_sequence: head,
            max_events: 100,
            wait_timeout_ms: Some(0),
        },
    )
    .await?;
    if !records.is_empty()
        || end.reason != StreamCeremonyEndReason::WaitElapsed as i32
        || end.resume_after_sequence != head
    {
        bail!("wait_timeout_ms=0 did not return an immediate caught-up End frame");
    }
    Ok(())
}

async fn follow_to_terminal(
    client: &mut MadeServiceClient<Channel>,
    after_sequence: u64,
) -> Result<Vec<CeremonyEventRecord>> {
    let mut live = client
        .stream_ceremony(StreamCeremonyRequest {
            ceremony_id: PROGRESS_ID.to_owned(),
            after_sequence,
            max_events: 100,
            wait_timeout_ms: Some(15_000),
        })
        .await
        .context("StreamCeremony failed to open the live stream")?
        .into_inner();
    let mut driver = client.clone();
    let drive = tokio::spawn(async move { drive_to_terminal(&mut driver).await });
    let mut records = Vec::new();
    let end = loop {
        let response = live
            .next()
            .await
            .context("live progress stream closed without End")?
            .context("live progress stream returned an RPC error")?;
        match response
            .frame
            .context("live progress response has no frame")?
        {
            Frame::Record(record) => records.push(record),
            Frame::End(end) => break end,
        }
    };
    drive.await.context("progress driver task panicked")??;
    if records.is_empty()
        || end.reason != StreamCeremonyEndReason::Terminal as i32
        || records.last().map(|record| record.event_type.as_str()) != Some("ceremony_completed")
    {
        bail!("live progress did not deliver records before terminal End");
    }
    Ok(records)
}

async fn assert_resumable_replay(
    client: &mut MadeServiceClient<Channel>,
    journal: &[CeremonyEventRecord],
) -> Result<StreamCeremonyEnd> {
    let (first_page, limited) = stream(
        client,
        StreamCeremonyRequest {
            ceremony_id: PROGRESS_ID.to_owned(),
            after_sequence: 0,
            max_events: 2,
            wait_timeout_ms: Some(0),
        },
    )
    .await?;
    if first_page.len() != 2
        || limited.reason != StreamCeremonyEndReason::EventLimit as i32
        || limited.resume_after_sequence != 2
    {
        bail!("max_events=2 did not return a resumable event-limit page");
    }
    let (resumed, terminal) = stream(
        client,
        StreamCeremonyRequest {
            ceremony_id: PROGRESS_ID.to_owned(),
            after_sequence: limited.resume_after_sequence,
            max_events: 1_000,
            wait_timeout_ms: Some(0),
        },
    )
    .await?;
    let replayed = first_page
        .into_iter()
        .chain(resumed)
        .map(|record| (record.sequence, record.event_id))
        .collect::<Vec<_>>();
    let expected = journal
        .iter()
        .map(|record| (record.sequence, record.event_id.clone()))
        .collect::<Vec<_>>();
    if terminal.reason != StreamCeremonyEndReason::Terminal as i32
        || replayed != expected
        || terminal.resume_after_sequence != journal.len() as u64
        || terminal.head_sequence != journal.len() as u64
    {
        bail!("resumed progress frames differ from ReadCeremonyEvents");
    }
    Ok(terminal)
}

async fn assert_terminal_journal(
    client: &mut MadeServiceClient<Channel>,
    head: u64,
    record_count: usize,
) -> Result<()> {
    let instance = client
        .get_ceremony_instance(GetCeremonyInstanceRequest {
            ceremony_id: PROGRESS_ID.to_owned(),
        })
        .await
        .context("GetCeremonyInstance failed for progress scenario")?
        .into_inner()
        .instance
        .context("progress instance response is empty")?;
    let journal = client
        .verify_ceremony_journal(VerifyCeremonyJournalRequest {
            ceremony_id: PROGRESS_ID.to_owned(),
        })
        .await
        .context("VerifyCeremonyJournal failed for progress scenario")?
        .into_inner();
    if !instance.completed
        || instance.current_state != "DONE"
        || !journal.intact
        || journal.head_version != head
        || usize::try_from(journal.record_count).unwrap_or(usize::MAX) != record_count
    {
        bail!("progress terminal, stream head, and verified journal differ");
    }
    Ok(())
}

async fn publish(client: &mut MadeServiceClient<Channel>, yaml: &str) -> Result<()> {
    let response = client
        .publish_ceremony_definition(PublishCeremonyDefinitionRequest {
            definition_yaml: yaml.to_owned(),
        })
        .await
        .context("PublishCeremonyDefinition failed for progress fixture")?
        .into_inner();
    if response.outcome != "published" && response.outcome != "already_published" {
        bail!(
            "progress fixture publication outcome was {}",
            response.outcome
        );
    }
    Ok(())
}

async fn drive_to_terminal(client: &mut MadeServiceClient<Channel>) -> Result<()> {
    client
        .run_ceremony_step(RunCeremonyStepRequest {
            ceremony_id: PROGRESS_ID.to_owned(),
            step_id: "review".to_owned(),
            lease_owner_id: "progress-e2e-runner".to_owned(),
            idempotency_key: "progress-e2e-review".to_owned(),
            lease_ttl_ms: 60_000,
            actor_kind: "service".to_owned(),
        })
        .await
        .context("RunCeremonyStep failed while progress stream was open")?;
    client
        .apply_ceremony_transition(ApplyCeremonyTransitionRequest {
            ceremony_id: PROGRESS_ID.to_owned(),
            trigger: "finish_review".to_owned(),
            actor_kind: "service".to_owned(),
        })
        .await
        .context("ApplyCeremonyTransition failed while progress stream was open")?;
    Ok(())
}

async fn stream(
    client: &mut MadeServiceClient<Channel>,
    request: StreamCeremonyRequest,
) -> Result<(Vec<CeremonyEventRecord>, StreamCeremonyEnd)> {
    let mut stream = client
        .stream_ceremony(request)
        .await
        .context("StreamCeremony failed")?
        .into_inner();
    let mut records = Vec::new();
    loop {
        let response = stream
            .next()
            .await
            .context("progress stream closed without End")?
            .context("progress stream returned an RPC error")?;
        match response.frame.context("progress response has no frame")? {
            Frame::Record(record) => records.push(record),
            Frame::End(end) => return Ok((records, end)),
        }
    }
}

async fn read_events(client: &mut MadeServiceClient<Channel>) -> Result<Vec<CeremonyEventRecord>> {
    let response = client
        .read_ceremony_events(ReadCeremonyEventsRequest {
            ceremony_id: PROGRESS_ID.to_owned(),
            from_version: 0,
            limit: 1_000,
        })
        .await
        .context("ReadCeremonyEvents failed for progress scenario")?
        .into_inner();
    if response.records.len() as u64 != response.head_version {
        bail!("progress event read did not reach head");
    }
    Ok(response.records)
}

async fn assert_report(client: &mut MadeServiceClient<Channel>) -> Result<()> {
    let report = client
        .generate_ceremony_report(GenerateCeremonyReportRequest {
            ceremony_ids: vec![PROGRESS_ID.to_owned()],
            title: "Compose progress report".to_owned(),
        })
        .await
        .context("GenerateCeremonyReport failed for progress scenario")?
        .into_inner();
    if report.completed_count != 1
        || report.incomplete_count != 0
        || !report.report_markdown.contains(PROGRESS_ID)
    {
        bail!("progress report does not describe the terminal ceremony");
    }
    Ok(())
}

fn string_context(key: &str, value: &str) -> Struct {
    Struct {
        fields: BTreeMap::from([(
            key.to_owned(),
            Value {
                kind: Some(Kind::StringValue(value.to_owned())),
            },
        )]),
    }
}
