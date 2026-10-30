use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use futures::StreamExt;
use made_proto::v1::stream_protocol_response::Frame;
use made_proto::v1::{
    ApplyProtocolTransitionRequest, GenerateProtocolReportRequest, GetProtocolInstanceRequest,
    ProtocolEventRecord, PublishProtocolDefinitionRequest, ReadProtocolEventsRequest,
    RunProtocolStepRequest, StartPublishedProtocolRequest, StreamProtocolEnd,
    StreamProtocolEndReason, StreamProtocolRequest, VerifyProtocolJournalRequest,
};
use prost_types::{value::Kind, Struct, Value};
use tracing::info;

use super::children_protocol_definitions::ChildrenProtocolDefinitions;

const PROGRESS_ID: &str = "e2e-protocol-progress";

pub(crate) async fn verify_live_protocol_progress(
    client: &mut crate::scenarios::E2eClient,
) -> Result<()> {
    publish(client, ChildrenProtocolDefinitions::child()).await?;
    client
        .start_published_protocol(StartPublishedProtocolRequest {
            protocol_id: PROGRESS_ID.to_owned(),
            protocol: "children_review".to_owned(),
            version: "1.0".to_owned(),
            context: Some(string_context(
                "review_brief",
                "Exercise the public protocol progress stream.",
            )),
            actor_id: "progress-e2e-operator".to_owned(),
            actor_kind: "service".to_owned(),
            budget_limits: None,
        })
        .await
        .context("StartPublishedProtocol failed for progress scenario")?;

    let initial = read_events(client).await?;
    let initial_head = initial.len() as u64;
    assert_idle_at_head(client, initial_head).await?;
    let live_records = follow_to_terminal(client, initial_head).await?;

    let all_records = read_events(client).await?;
    let terminal = assert_resumable_replay(client, &all_records).await?;
    assert_terminal_journal(client, terminal.head_sequence, all_records.len()).await?;
    assert_report(client).await?;

    info!(
        protocol_id = PROGRESS_ID,
        live_records = live_records.len(),
        journal_records = all_records.len(),
        "live progress, bounded replay, resume, and journal agree"
    );
    Ok(())
}

async fn assert_idle_at_head(client: &mut crate::scenarios::E2eClient, head: u64) -> Result<()> {
    let (records, end) = stream(
        client,
        StreamProtocolRequest {
            protocol_id: PROGRESS_ID.to_owned(),
            after_sequence: head,
            max_events: 100,
            wait_timeout_ms: Some(0),
            ..Default::default()
        },
    )
    .await?;
    if !records.is_empty()
        || end.reason != StreamProtocolEndReason::WaitElapsed as i32
        || end.resume_after_sequence != head
    {
        bail!("wait_timeout_ms=0 did not return an immediate caught-up End frame");
    }
    Ok(())
}

async fn follow_to_terminal(
    client: &mut crate::scenarios::E2eClient,
    after_sequence: u64,
) -> Result<Vec<ProtocolEventRecord>> {
    let mut live = client
        .stream_protocol(StreamProtocolRequest {
            protocol_id: PROGRESS_ID.to_owned(),
            after_sequence,
            max_events: 100,
            wait_timeout_ms: Some(15_000),
            ..Default::default()
        })
        .await
        .context("StreamProtocol failed to open the live stream")?
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
            Frame::AgentSnapshot(_) | Frame::AgentActivity(_) => {
                bail!("plain live progress returned unrequested agent activity")
            }
        }
    };
    drive.await.context("progress driver task panicked")??;
    if records.is_empty()
        || end.reason != StreamProtocolEndReason::Terminal as i32
        || records.last().map(|record| record.event_type.as_str()) != Some("protocol_completed")
    {
        bail!("live progress did not deliver records before terminal End");
    }
    Ok(records)
}

async fn assert_resumable_replay(
    client: &mut crate::scenarios::E2eClient,
    journal: &[ProtocolEventRecord],
) -> Result<StreamProtocolEnd> {
    let (first_page, limited) = stream(
        client,
        StreamProtocolRequest {
            protocol_id: PROGRESS_ID.to_owned(),
            after_sequence: 0,
            max_events: 2,
            wait_timeout_ms: Some(0),
            ..Default::default()
        },
    )
    .await?;
    if first_page.len() != 2
        || limited.reason != StreamProtocolEndReason::EventLimit as i32
        || limited.resume_after_sequence != 2
    {
        bail!("max_events=2 did not return a resumable event-limit page");
    }
    let (resumed, terminal) = stream(
        client,
        StreamProtocolRequest {
            protocol_id: PROGRESS_ID.to_owned(),
            after_sequence: limited.resume_after_sequence,
            max_events: 1_000,
            wait_timeout_ms: Some(0),
            ..Default::default()
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
    if terminal.reason != StreamProtocolEndReason::Terminal as i32
        || replayed != expected
        || terminal.resume_after_sequence != journal.len() as u64
        || terminal.head_sequence != journal.len() as u64
    {
        bail!("resumed progress frames differ from ReadProtocolEvents");
    }
    Ok(terminal)
}

async fn assert_terminal_journal(
    client: &mut crate::scenarios::E2eClient,
    head: u64,
    record_count: usize,
) -> Result<()> {
    let instance = client
        .get_protocol_instance(GetProtocolInstanceRequest {
            protocol_id: PROGRESS_ID.to_owned(),
        })
        .await
        .context("GetProtocolInstance failed for progress scenario")?
        .into_inner()
        .instance
        .context("progress instance response is empty")?;
    let journal = client
        .verify_protocol_journal(VerifyProtocolJournalRequest {
            protocol_id: PROGRESS_ID.to_owned(),
        })
        .await
        .context("VerifyProtocolJournal failed for progress scenario")?
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

async fn publish(client: &mut crate::scenarios::E2eClient, yaml: &str) -> Result<()> {
    let response = client
        .publish_protocol_definition(PublishProtocolDefinitionRequest {
            definition_yaml: yaml.to_owned(),
        })
        .await
        .context("PublishProtocolDefinition failed for progress fixture")?
        .into_inner();
    if response.outcome != "published" && response.outcome != "already_published" {
        bail!(
            "progress fixture publication outcome was {}",
            response.outcome
        );
    }
    Ok(())
}

async fn drive_to_terminal(client: &mut crate::scenarios::E2eClient) -> Result<()> {
    client
        .run_protocol_step(RunProtocolStepRequest {
            protocol_id: PROGRESS_ID.to_owned(),
            step_id: "review".to_owned(),
            lease_owner_id: "progress-e2e-runner".to_owned(),
            idempotency_key: "progress-e2e-review".to_owned(),
            lease_ttl_ms: 60_000,
            actor_kind: "service".to_owned(),
        })
        .await
        .context("RunProtocolStep failed while progress stream was open")?;
    client
        .apply_protocol_transition(ApplyProtocolTransitionRequest {
            protocol_id: PROGRESS_ID.to_owned(),
            trigger: "finish_review".to_owned(),
            actor_kind: "service".to_owned(),
        })
        .await
        .context("ApplyProtocolTransition failed while progress stream was open")?;
    Ok(())
}

async fn stream(
    client: &mut crate::scenarios::E2eClient,
    request: StreamProtocolRequest,
) -> Result<(Vec<ProtocolEventRecord>, StreamProtocolEnd)> {
    let mut stream = client
        .stream_protocol(request)
        .await
        .context("StreamProtocol failed")?
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
            Frame::AgentSnapshot(_) | Frame::AgentActivity(_) => {
                bail!("plain progress scenario received unrequested agent activity")
            }
        }
    }
}

async fn read_events(client: &mut crate::scenarios::E2eClient) -> Result<Vec<ProtocolEventRecord>> {
    let response = client
        .read_protocol_events(ReadProtocolEventsRequest {
            protocol_id: PROGRESS_ID.to_owned(),
            from_version: 0,
            limit: 1_000,
        })
        .await
        .context("ReadProtocolEvents failed for progress scenario")?
        .into_inner();
    if response.records.len() as u64 != response.head_version {
        bail!("progress event read did not reach head");
    }
    Ok(response.records)
}

async fn assert_report(client: &mut crate::scenarios::E2eClient) -> Result<()> {
    let report = client
        .generate_protocol_report(GenerateProtocolReportRequest {
            protocol_ids: vec![PROGRESS_ID.to_owned()],
            title: "Compose progress report".to_owned(),
        })
        .await
        .context("GenerateProtocolReport failed for progress scenario")?
        .into_inner();
    if report.completed_count != 1
        || report.incomplete_count != 0
        || !report.report_markdown.contains(PROGRESS_ID)
    {
        bail!("progress report does not describe the terminal protocol");
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
