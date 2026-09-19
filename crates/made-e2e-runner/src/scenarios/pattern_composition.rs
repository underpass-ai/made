use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use made_proto::v1::{
    CeremonyEventRecord, GenerateCeremonyReportRequest, ReadCeremonyEventsRequest,
    RunCeremonyRequest, RunCeremonyResponse,
};
use prost_types::{value::Kind, Struct, Value};
use tracing::info;

use super::ceremony_vllm_provider_config::CeremonyVllmProviderConfig;
use super::pattern_ceremony_definition::PatternCeremonyDefinition;

const CONCURRENT_ID: &str = "e2e-concurrent-review-pattern";
const INCIDENT_ID: &str = "e2e-incident-review-pattern";
const REVIEW_STEPS: [&str; 3] = ["review_correctness", "review_security", "review_operations"];

pub(crate) async fn verify_concurrent_review_pattern(
    client: &mut crate::scenarios::E2eClient,
) -> Result<()> {
    let provider = CeremonyVllmProviderConfig::from_env()?;
    let definition = PatternCeremonyDefinition::concurrent_review(&provider)?;
    let response = run(client, CONCURRENT_ID, definition.as_yaml()).await?;
    assert_completed(&response, "COMPLETED", 4)?;

    let records = read_events(client, CONCURRENT_ID).await?;
    assert_claimed_fanout_before_completion(&records)?;
    assert_completed_steps(&records, &REVIEW_STEPS)?;
    assert_report(
        client,
        CONCURRENT_ID,
        &[
            "review_correctness",
            "review_security",
            "review_operations",
            "synthesize_reviews",
        ],
    )
    .await?;

    info!(
        ceremony_id = CONCURRENT_ID,
        events = records.len(),
        "concurrent-review ran through public RPC with claimed fan-out"
    );
    Ok(())
}

pub(crate) async fn verify_incident_review_pattern(
    client: &mut crate::scenarios::E2eClient,
) -> Result<()> {
    let provider = CeremonyVllmProviderConfig::from_env()?;
    let definition = PatternCeremonyDefinition::incident_review(&provider)?;
    let response = run(client, INCIDENT_ID, definition.as_yaml()).await?;
    assert_completed(&response, "COMPLETED", 9)?;

    let records = read_events(client, INCIDENT_ID).await?;
    assert_projected_bool(&records, "writeup_check_1", false)?;
    assert_projected_bool(&records, "writeup_check_2", true)?;
    if records.iter().any(|record| {
        record.event_type == "step_started"
            && event_string(record, "step_id") == Some("writeup_fallback")
    }) {
        bail!("incident review unexpectedly entered its capped fallback after approval");
    }
    assert_report(
        client,
        INCIDENT_ID,
        &[
            "analysis_review_1",
            "analysis_review_2",
            "writeup_check_1",
            "writeup_check_2",
        ],
    )
    .await?;

    info!(
        ceremony_id = INCIDENT_ID,
        events = records.len(),
        "incident_review composed broadcast and maker-checker stages through public RPC"
    );
    Ok(())
}

async fn run(
    client: &mut crate::scenarios::E2eClient,
    ceremony_id: &str,
    definition_yaml: &str,
) -> Result<RunCeremonyResponse> {
    client
        .run_ceremony(RunCeremonyRequest {
            actor_id: "pattern-e2e-operator".to_owned(),
            actor_kind: "service".to_owned(),
            ceremony_id: ceremony_id.to_owned(),
            definition_yaml: definition_yaml.to_owned(),
            context: Some(Struct {
                fields: BTreeMap::from([(
                    "draft".to_owned(),
                    Value {
                        kind: Some(Kind::StringValue(
                            "Review the deterministic Compose fixture.".to_owned(),
                        )),
                    },
                )]),
            }),
            lease_owner_id: "pattern-e2e-runner".to_owned(),
            lease_ttl_ms: 60_000,
        })
        .await
        .with_context(|| format!("RunCeremony failed for {ceremony_id}"))
        .map(tonic::Response::into_inner)
}

fn assert_completed(response: &RunCeremonyResponse, state: &str, steps: usize) -> Result<()> {
    if !response.completed || response.final_state != state {
        bail!(
            "pattern ceremony did not complete in {state}: completed={}, final_state={}",
            response.completed,
            response.final_state
        );
    }
    if response.steps.len() != steps {
        bail!(
            "pattern ceremony expected {steps} completed steps, got {}",
            response.steps.len()
        );
    }
    if response.steps.iter().any(|step| step.status != "COMPLETED") {
        bail!("pattern ceremony response contains a non-completed step");
    }
    Ok(())
}

async fn read_events(
    client: &mut crate::scenarios::E2eClient,
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
        bail!(
            "event read did not reach head: records={}, head={}",
            response.records.len(),
            response.head_version
        );
    }
    Ok(response.records)
}

fn assert_claimed_fanout_before_completion(records: &[CeremonyEventRecord]) -> Result<()> {
    let first_completion = records
        .iter()
        .filter(|record| {
            record.event_type == "step_completed"
                && event_string(record, "step_id").is_some_and(|step| REVIEW_STEPS.contains(&step))
        })
        .map(|record| record.sequence)
        .min()
        .context("concurrent review stream has no reviewer completion")?;
    for step in REVIEW_STEPS {
        let started = records
            .iter()
            .find(|record| {
                record.event_type == "step_started" && event_string(record, "step_id") == Some(step)
            })
            .with_context(|| format!("concurrent review stream has no start for {step}"))?;
        if started.sequence >= first_completion {
            bail!(
                "reviewer {step} was not claimed before the first sibling completion: start={}, completion={first_completion}",
                started.sequence
            );
        }
    }
    Ok(())
}

fn assert_completed_steps(records: &[CeremonyEventRecord], steps: &[&str]) -> Result<()> {
    for step in steps {
        if !records.iter().any(|record| {
            record.event_type == "step_completed" && event_string(record, "step_id") == Some(step)
        }) {
            bail!("event stream is missing completion for {step}");
        }
    }
    Ok(())
}

fn assert_projected_bool(
    records: &[CeremonyEventRecord],
    step_id: &str,
    expected: bool,
) -> Result<()> {
    let record = records
        .iter()
        .find(|record| {
            record.event_type == "step_completed"
                && event_string(record, "step_id") == Some(step_id)
        })
        .with_context(|| format!("event stream is missing completion for {step_id}"))?;
    let actual = event_struct(record)
        .and_then(|event| struct_field(event, "result"))
        .and_then(value_as_struct)
        .and_then(|result| struct_field(result, "output"))
        .and_then(value_as_struct)
        .and_then(|output| struct_field(output, "approved"))
        .and_then(value_as_bool);
    if actual != Some(expected) {
        bail!("{step_id} projected approved={actual:?}, expected {expected}");
    }
    Ok(())
}

async fn assert_report(
    client: &mut crate::scenarios::E2eClient,
    ceremony_id: &str,
    expected_steps: &[&str],
) -> Result<()> {
    let title = "Compose pattern report";
    let report = client
        .generate_ceremony_report(GenerateCeremonyReportRequest {
            ceremony_ids: vec![ceremony_id.to_owned()],
            title: title.to_owned(),
        })
        .await
        .with_context(|| format!("GenerateCeremonyReport failed for {ceremony_id}"))?
        .into_inner();
    if report.ceremony_ids.as_slice() != [ceremony_id] || report.ceremony_count != 1 {
        bail!("pattern report does not identify exactly {ceremony_id}");
    }
    if report.completed_count != 1 || report.incomplete_count != 0 {
        bail!(
            "pattern report completion counts are wrong: completed={}, incomplete={}",
            report.completed_count,
            report.incomplete_count
        );
    }
    for expected in std::iter::once(title)
        .chain(std::iter::once(ceremony_id))
        .chain(expected_steps.iter().copied())
    {
        if !report.report_markdown.contains(expected) {
            bail!("pattern report is missing `{expected}`");
        }
    }
    Ok(())
}

fn event_struct(record: &CeremonyEventRecord) -> Option<&Struct> {
    record.event.as_ref()
}

fn event_string<'a>(record: &'a CeremonyEventRecord, field: &str) -> Option<&'a str> {
    event_struct(record)
        .and_then(|event| struct_field(event, field))
        .and_then(value_as_str)
}

fn struct_field<'a>(value: &'a Struct, field: &str) -> Option<&'a Value> {
    value.fields.get(field)
}

fn value_as_struct(value: &Value) -> Option<&Struct> {
    match value.kind.as_ref()? {
        Kind::StructValue(value) => Some(value),
        _ => None,
    }
}

fn value_as_str(value: &Value) -> Option<&str> {
    match value.kind.as_ref()? {
        Kind::StringValue(value) => Some(value),
        _ => None,
    }
}

fn value_as_bool(value: &Value) -> Option<bool> {
    match value.kind.as_ref()? {
        Kind::BoolValue(value) => Some(*value),
        _ => None,
    }
}
