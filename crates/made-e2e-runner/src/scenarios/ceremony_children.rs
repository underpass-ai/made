use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    ApplyCeremonyTransitionRequest, CeremonyEventRecord, CeremonyInstanceState,
    GenerateCeremonyReportRequest, GetCeremonyInstanceRequest, PublishCeremonyDefinitionRequest,
    ReadCeremonyEventsRequest, RunCeremonyStepRequest, StartPublishedCeremonyRequest,
    VerifyCeremonyJournalRequest,
};
use prost_types::{value::Kind, Struct, Value};
use tonic::transport::Channel;
use tracing::info;

use super::ceremony_vllm_provider_config::CeremonyVllmProviderConfig;
use super::children_ceremony_definitions::ChildrenCeremonyDefinitions;

const PARENT_ID: &str = "e2e-children-parent";
const SPAWN_STEP: &str = "spawn_reviews";

pub(crate) async fn verify_durable_children_over_public_rpc(
    client: &mut MadeServiceClient<Channel>,
) -> Result<()> {
    let definitions = ChildrenCeremonyDefinitions::new(&CeremonyVllmProviderConfig::from_env()?)?;
    publish(client, definitions.child()).await?;
    publish(client, definitions.parent()).await?;

    let started = client
        .start_published_ceremony(StartPublishedCeremonyRequest {
            ceremony_id: PARENT_ID.to_owned(),
            ceremony: "children_parent".to_owned(),
            version: "1.0".to_owned(),
            context: Some(string_context(
                "brief",
                "Review this durable parent fixture.",
            )),
            actor_id: "children-e2e-operator".to_owned(),
            actor_kind: "service".to_owned(),
        })
        .await
        .context("StartPublishedCeremony failed for the parent")?
        .into_inner()
        .instance
        .context("parent start response has no instance")?;
    if started.completed || started.next_step_id != SPAWN_STEP {
        bail!("parent did not open at its spawn step");
    }

    let spawned = client
        .run_ceremony_step(RunCeremonyStepRequest {
            ceremony_id: PARENT_ID.to_owned(),
            step_id: SPAWN_STEP.to_owned(),
            lease_owner_id: "children-e2e-runner".to_owned(),
            idempotency_key: "children-e2e-spawn".to_owned(),
            lease_ttl_ms: 60_000,
            actor_kind: "service".to_owned(),
        })
        .await
        .context("RunCeremonyStep failed for the parent spawn")?
        .into_inner()
        .instance
        .context("spawn response has no parent instance")?;
    let group = spawned
        .child_groups
        .first()
        .context("spawned parent has no child group")?;
    let child_ids = group
        .children
        .iter()
        .map(|child| child.child_id.clone())
        .collect::<Vec<_>>();
    if child_ids.len() != 2 || child_ids[0] == child_ids[1] {
        bail!("parent did not fan out exactly two distinct children");
    }

    for (position, child_id) in child_ids.iter().enumerate() {
        let child = get_instance(client, child_id).await?;
        let lineage = child.lineage.context("opened child has no lineage")?;
        if lineage.parent_id != PARENT_ID || lineage.position != position as u32 {
            bail!("child {child_id} has unexpected sealed lineage");
        }
        run_child_to_terminal(client, child_id, position).await?;
    }

    let accepted = wait_for_parent_completions(client, child_ids.len()).await?;
    if accepted.child_groups[0].completions.len() != 2 {
        bail!("parent did not durably accept both child completions");
    }
    let parent = client
        .apply_ceremony_transition(ApplyCeremonyTransitionRequest {
            ceremony_id: PARENT_ID.to_owned(),
            trigger: "finish_parent".to_owned(),
            actor_kind: "service".to_owned(),
        })
        .await
        .context("ApplyCeremonyTransition failed for the parent")?
        .into_inner()
        .instance
        .context("parent transition response has no instance")?;
    if !parent.completed || parent.current_state != "DONE" {
        bail!("parent did not reach terminal DONE after both child completions");
    }

    let parent_records = read_events(client, PARENT_ID).await?;
    let accepted_events = parent_records
        .iter()
        .filter(|record| record.event_type == "child_completion_accepted")
        .count();
    if accepted_events != 2 {
        bail!("parent journal has {accepted_events} ChildCompletionAccepted events, expected 2");
    }
    verify_journal(client, PARENT_ID).await?;
    for child_id in &child_ids {
        verify_journal(client, child_id).await?;
    }
    assert_report(client, &child_ids).await?;

    info!(
        ceremony_id = PARENT_ID,
        children = ?child_ids,
        parent_events = parent_records.len(),
        "two durable children completed and were accepted by the NATS recovery cursor"
    );
    Ok(())
}

async fn publish(client: &mut MadeServiceClient<Channel>, yaml: &str) -> Result<()> {
    let response = client
        .publish_ceremony_definition(PublishCeremonyDefinitionRequest {
            definition_yaml: yaml.to_owned(),
        })
        .await
        .context("PublishCeremonyDefinition failed")?
        .into_inner();
    if response.outcome != "published" && response.outcome != "already_published" {
        bail!(
            "ceremony publication was not accepted: outcome={}",
            response.outcome
        );
    }
    Ok(())
}

async fn run_child_to_terminal(
    client: &mut MadeServiceClient<Channel>,
    child_id: &str,
    position: usize,
) -> Result<()> {
    let stepped = client
        .run_ceremony_step(RunCeremonyStepRequest {
            ceremony_id: child_id.to_owned(),
            step_id: "review".to_owned(),
            lease_owner_id: "children-e2e-runner".to_owned(),
            idempotency_key: format!("children-e2e-review-{position}"),
            lease_ttl_ms: 60_000,
            actor_kind: "service".to_owned(),
        })
        .await
        .with_context(|| format!("RunCeremonyStep failed for child {child_id}"))?
        .into_inner()
        .instance
        .context("child step response has no instance")?;
    if !stepped
        .steps
        .iter()
        .any(|step| step.step_id == "review" && step.status == "COMPLETED")
    {
        bail!("child {child_id} review did not complete");
    }
    let child = client
        .apply_ceremony_transition(ApplyCeremonyTransitionRequest {
            ceremony_id: child_id.to_owned(),
            trigger: "finish_review".to_owned(),
            actor_kind: "service".to_owned(),
        })
        .await
        .with_context(|| format!("ApplyCeremonyTransition failed for child {child_id}"))?
        .into_inner()
        .instance
        .context("child transition response has no instance")?;
    if !child.completed || child.current_state != "DONE" {
        bail!("child {child_id} did not reach terminal DONE");
    }
    Ok(())
}

async fn wait_for_parent_completions(
    client: &mut MadeServiceClient<Channel>,
    expected: usize,
) -> Result<CeremonyInstanceState> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let parent = get_instance(client, PARENT_ID).await?;
        if parent
            .child_groups
            .first()
            .is_some_and(|group| group.completions.len() == expected)
        {
            return Ok(parent);
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("NATS recovery cursor did not accept {expected} child completions");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn get_instance(
    client: &mut MadeServiceClient<Channel>,
    ceremony_id: &str,
) -> Result<CeremonyInstanceState> {
    client
        .get_ceremony_instance(GetCeremonyInstanceRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .with_context(|| format!("GetCeremonyInstance failed for {ceremony_id}"))?
        .into_inner()
        .instance
        .with_context(|| format!("GetCeremonyInstance returned no {ceremony_id} instance"))
}

async fn read_events(
    client: &mut MadeServiceClient<Channel>,
    ceremony_id: &str,
) -> Result<Vec<CeremonyEventRecord>> {
    let response = client
        .read_ceremony_events(ReadCeremonyEventsRequest {
            ceremony_id: ceremony_id.to_owned(),
            from_version: 0,
            limit: 1_000,
        })
        .await
        .with_context(|| format!("ReadCeremonyEvents failed for {ceremony_id}"))?
        .into_inner();
    if response.records.len() as u64 != response.head_version {
        bail!("event read for {ceremony_id} did not reach its head");
    }
    Ok(response.records)
}

async fn verify_journal(client: &mut MadeServiceClient<Channel>, ceremony_id: &str) -> Result<()> {
    let journal = client
        .verify_ceremony_journal(VerifyCeremonyJournalRequest {
            ceremony_id: ceremony_id.to_owned(),
        })
        .await
        .with_context(|| format!("VerifyCeremonyJournal failed for {ceremony_id}"))?
        .into_inner();
    if !journal.intact || u64::from(journal.record_count) != journal.head_version {
        bail!(
            "journal for {ceremony_id} is not intact: {}",
            journal.reason
        );
    }
    Ok(())
}

async fn assert_report(
    client: &mut MadeServiceClient<Channel>,
    child_ids: &[String],
) -> Result<()> {
    let ceremony_ids = std::iter::once(PARENT_ID.to_owned())
        .chain(child_ids.iter().cloned())
        .collect::<Vec<_>>();
    let report = client
        .generate_ceremony_report(GenerateCeremonyReportRequest {
            ceremony_ids: ceremony_ids.clone(),
            title: "Compose durable children report".to_owned(),
        })
        .await
        .context("GenerateCeremonyReport failed for the parent and children")?
        .into_inner();
    if report.ceremony_ids != ceremony_ids
        || report.ceremony_count != 3
        || report.completed_count != 3
        || report.incomplete_count != 0
    {
        bail!("children report does not describe three completed ceremonies");
    }
    for ceremony_id in &report.ceremony_ids {
        if !report.report_markdown.contains(ceremony_id) {
            bail!("children report is missing {ceremony_id}");
        }
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
