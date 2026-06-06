//! Chain 2 — "consumer handoff report" path.
//!
//! Drives the choreographer through the Strict-mode contract path:
//!
//! 1. Load the canonical Report JSON Schema (from disk by default, or
//!    via the `run_chain_2_with_schema` overload that tests use).
//! 2. Register the corresponding `OutputContract` on the choreographer
//!    via `RegisterContract`.
//! 3. Call `RunCouncilDecision` with `validation_mode = STRICT`.
//! 4. Branch on the response:
//!    - Ok → schema-validate the winner's content. The positive path
//!      remains Skipped against a NoopAgent stack (free-form text
//!      cannot satisfy the schema); a stub-LLM follow-up will exercise it.
//!    - Err FailedPrecondition mentioning the contract id → the
//!      rejection path triggered; that IS the assertion this chain
//!      makes against the current compose stack.
//!    - Other Err → both report assertions are recorded as Failed.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use choreo_proto::v1 as pb;
use choreo_proto::v1::run_council_decision_request::Selector;
use jsonschema::JSONSchema;
use tonic::Code;
use tracing::info;

use crate::bundle::deterministic_bundle;
use crate::outcome::{AssertionRecord, ChainOutcome};
use crate::{Harness, HarnessConfig};

pub(crate) const DEFAULT_REPORT_SCHEMA_PATH: &str =
    "api/examples/output-contracts/report.schema.json";

/// Read the Report schema from the path given by
/// `CHOREO_REPORT_SCHEMA_PATH` (falling back to the in-repo default)
/// and then call [`run_chain_2_with_schema`].
///
/// Tests should prefer [`run_chain_2_with_schema`] directly so they
/// don't have to mutate process env (env mutation leaks across
/// concurrent tests).
pub async fn run_chain_2(h: &mut Harness, cfg: &HarnessConfig) -> Result<ChainOutcome> {
    let path = std::env::var("CHOREO_REPORT_SCHEMA_PATH")
        .unwrap_or_else(|_| DEFAULT_REPORT_SCHEMA_PATH.to_owned());
    match std::fs::read_to_string(&path) {
        Ok(schema) => run_chain_2_with_schema(h, cfg, &schema).await,
        Err(err) => {
            // The binary still needs a printable outcome — record a
            // single Failed assertion and a Skipped Pair instead of
            // bubbling the IO error.
            let detail = format!("schema not found at {path}: {err}");
            let assertions = vec![
                AssertionRecord::failed("report_schema_registered", detail, Duration::ZERO),
                AssertionRecord::skipped(
                    "report_contract_rejects_freeform_text",
                    "schema-registration step failed; downstream assertions did not run",
                ),
                AssertionRecord::skipped(
                    "report_payload_validates",
                    "schema-registration step failed; downstream assertions did not run",
                ),
            ];
            Ok(ChainOutcome {
                chain: "chain2",
                contract_id: cfg.contract_id.clone(),
                task_id: None,
                winner_proposal_id: None,
                validation_passed: None,
                assertions,
                bus_envelopes: vec![],
                total_duration: Duration::ZERO,
            })
        }
    }
}

/// Test-facing overload that takes the Report schema directly. Avoids
/// the process-env mutation that the file-loading wrapper does, so
/// tests can run concurrently without serialising on a shared lock.
#[allow(clippy::too_many_lines)] // a single chain run linearises register → invoke → branch; splitting fragments the flow
pub async fn run_chain_2_with_schema(
    h: &mut Harness,
    cfg: &HarnessConfig,
    schema_json: &str,
) -> Result<ChainOutcome> {
    let start = Instant::now();
    let mut assertions: Vec<AssertionRecord> = Vec::new();

    // 1. Register the contract. The compose stack may have pre-seeded
    //    it already (via CHOREO_CONTRACT_DIR), in which case the
    //    server responds with AlreadyExists / FailedPrecondition —
    //    treat both as "the contract IS registered, move on".
    let register_start = Instant::now();
    let register_req = pb::RegisterContractRequest {
        contract: Some(pb::OutputContract {
            contract_id: cfg.contract_id.clone(),
            format: pb::OutputFormat::JsonObject as i32,
            fields: HashMap::new(),
            json_schema: schema_json.to_owned(),
        }),
    };
    let register_result = h.grpc.register_contract(register_req).await;
    let register_elapsed = register_start.elapsed();

    match register_result {
        Ok(_) => {
            assertions.push(AssertionRecord::passed(
                "report_schema_registered",
                register_elapsed,
            ));
        }
        Err(status)
            if status.code() == Code::AlreadyExists
                || status.code() == Code::FailedPrecondition =>
        {
            // The server reports the contract already exists. From
            // the consumer's point of view, the contract IS in the
            // registry, so the assertion still holds.
            assertions.push(AssertionRecord::passed(
                "report_schema_registered",
                register_elapsed,
            ));
        }
        Err(status) => {
            assertions.push(AssertionRecord::failed(
                "report_schema_registered",
                format!("RegisterContract returned {status:?}"),
                register_elapsed,
            ));
            assertions.push(AssertionRecord::skipped(
                "report_contract_rejects_freeform_text",
                "schema-registration step failed; downstream assertions did not run",
            ));
            assertions.push(AssertionRecord::skipped(
                "report_payload_validates",
                "schema-registration step failed; downstream assertions did not run",
            ));
            return Ok(ChainOutcome {
                chain: "chain2",
                contract_id: cfg.contract_id.clone(),
                task_id: None,
                winner_proposal_id: None,
                validation_passed: None,
                assertions,
                bus_envelopes: vec![],
                total_duration: start.elapsed(),
            });
        }
    }

    // 2. Run a Strict-mode decision.
    let rpc_start = Instant::now();
    let request = pb::RunCouncilDecisionRequest {
        contract_id: cfg.contract_id.clone(),
        external_context: Some(deterministic_bundle()),
        validation_mode: pb::ValidationMode::Strict as i32,
        metadata: None,
        description: "consumer-smoke chain 2 handoff report".to_owned(),
        selector: Some(Selector::Specialty(cfg.specialty.clone())),
    };
    let response = h.grpc.run_council_decision(request).await;
    let rpc_elapsed = rpc_start.elapsed();

    let (task_id, winner_proposal_id, validation_passed) = match response {
        Ok(resp) => {
            let resp = resp.into_inner();
            let task_id = if resp.task_id.is_empty() {
                None
            } else {
                Some(resp.task_id.clone())
            };
            let winner_proposal_id = resp
                .winner
                .as_ref()
                .and_then(|d| d.proposal.as_ref())
                .map(|p| p.proposal_id.clone());
            let validation_passed = resp.validation.as_ref().map(|v| v.passed);

            // Positive path: the council returned a winner under
            // Strict mode. That requires structured-JSON output the
            // schema accepts. Validate it here.
            let content = resp
                .winner
                .as_ref()
                .and_then(|d| d.proposal.as_ref())
                .map(|p| p.content.clone())
                .unwrap_or_default();
            assertions.push(AssertionRecord::skipped(
                "report_contract_rejects_freeform_text",
                "positive path reached; rejection assertion not exercised",
            ));
            assertions.push(validate_payload_against_schema(
                schema_json,
                &content,
                rpc_elapsed,
            ));
            (task_id, winner_proposal_id, validation_passed)
        }
        Err(status)
            if status.code() == Code::FailedPrecondition
                && status.message().contains(&cfg.contract_id) =>
        {
            // Rejection path: this IS the assertion the chain is
            // designed to make against the current NoopAgent compose
            // stack — free-form text cannot satisfy the Report schema.
            assertions.push(AssertionRecord::passed(
                "report_contract_rejects_freeform_text",
                rpc_elapsed,
            ));
            assertions.push(AssertionRecord::skipped(
                "report_payload_validates",
                "rejection path reached; positive validation requires a structured-JSON agent (stub-LLM not deployed)",
            ));
            (None, None, None)
        }
        Err(status) => {
            let detail = format!("{status:?}");
            assertions.push(AssertionRecord::failed(
                "report_contract_rejects_freeform_text",
                detail.clone(),
                rpc_elapsed,
            ));
            assertions.push(AssertionRecord::failed(
                "report_payload_validates",
                detail,
                Duration::ZERO,
            ));
            (None, None, None)
        }
    };

    let outcome = ChainOutcome {
        chain: "chain2",
        contract_id: cfg.contract_id.clone(),
        task_id,
        winner_proposal_id,
        validation_passed,
        assertions,
        bus_envelopes: vec![],
        total_duration: start.elapsed(),
    };
    info!(
        chain = outcome.chain,
        passed = outcome.passed(),
        passed_count = outcome.passed_count(),
        failed_count = outcome.failed_count(),
        skipped_count = outcome.skipped_count(),
        "consumer-smoke chain2 finished"
    );
    Ok(outcome)
}

/// Validate `content` (a JSON string) against `schema_json` and
/// produce the `report_payload_validates` assertion.
///
/// Implementation note: `JSONSchema::compile` borrows from its input
/// `serde_json::Value`, so this function owns the schema value and
/// the payload value locally and runs the compile+validate in one
/// scope; that keeps the API of this helper simple (just `&str` in,
/// `AssertionRecord` out) at the cost of a slightly nested control
/// flow.
pub(crate) fn validate_payload_against_schema(
    schema_json: &str,
    content: &str,
    duration: Duration,
) -> AssertionRecord {
    let schema_value: serde_json::Value = match serde_json::from_str(schema_json) {
        Ok(v) => v,
        Err(err) => {
            return AssertionRecord::failed(
                "report_payload_validates",
                format!("invalid schema JSON: {err}"),
                duration,
            );
        }
    };
    let payload: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(err) => {
            return AssertionRecord::failed(
                "report_payload_validates",
                format!("winner content is not JSON: {err}"),
                duration,
            );
        }
    };
    match JSONSchema::compile(&schema_value) {
        Err(err) => AssertionRecord::failed(
            "report_payload_validates",
            format!("could not compile schema: {err}"),
            duration,
        ),
        Ok(compiled) => {
            if compiled.is_valid(&payload) {
                AssertionRecord::passed("report_payload_validates", duration)
            } else {
                let errs: Vec<String> = compiled
                    .validate(&payload)
                    .err()
                    .into_iter()
                    .flat_map(|it| it.map(|e| format!("{e}")))
                    .collect();
                AssertionRecord::failed(
                    "report_payload_validates",
                    format!("schema mismatch: {}", errs.join("; ")),
                    duration,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TINY_SCHEMA: &str = r#"{
        "type": "object",
        "required": ["id"],
        "properties": { "id": { "type": "string" } },
        "additionalProperties": false
    }"#;

    #[test]
    fn payload_validation_passes_on_match() {
        let r = validate_payload_against_schema(TINY_SCHEMA, r#"{"id":"x"}"#, Duration::ZERO);
        assert!(r.is_passed(), "expected pass, got {:?}", r.status);
    }

    #[test]
    fn payload_validation_fails_on_mismatch() {
        let r = validate_payload_against_schema(TINY_SCHEMA, r#"{"id":42}"#, Duration::ZERO);
        assert!(r.is_failed());
    }

    #[test]
    fn payload_validation_fails_on_non_json_content() {
        let r = validate_payload_against_schema(TINY_SCHEMA, "not json", Duration::ZERO);
        assert!(r.is_failed());
    }
}
