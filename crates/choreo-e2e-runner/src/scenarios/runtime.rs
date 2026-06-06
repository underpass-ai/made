use anyhow::{anyhow, bail, Context, Result};
use choreo_proto::v1::choreographer_service_client::ChoreographerServiceClient;
use choreo_proto::v1::{OrchestrateRequest, Task};
use prost_types::value::Kind as PbKind;
use tonic::transport::Channel;
use tracing::info;

use super::pb_struct_from_pairs;

/// Drives `Orchestrate` on the seeded council with a task that carries
/// a `runtime.tool_name` attribute. The compose stack wires
/// `CHOREO_EXECUTOR_KIND=runtime` pointing at the in-stack
/// `stub-runtime` sidecar, so a successful outcome here proves the
/// full chain — Deliberate -> winner -> RuntimeExecutor -> gRPC to
/// `underpass.runtime.v1.{SessionService,InvocationService}` -> back
/// up the use-case -> `TaskCompleted`.
pub(crate) async fn verify_orchestrate_invokes_runtime_executor(
    client: &mut ChoreographerServiceClient<Channel>,
    specialty: &str,
) -> Result<()> {
    let attributes = pb_struct_from_pairs([(
        "runtime.tool_name",
        PbKind::StringValue("stub.echo".to_owned()),
    )]);

    let response = client
        .orchestrate(OrchestrateRequest {
            task: Some(Task {
                task_id: "e2e-task-5".to_owned(),
                specialty: specialty.to_owned(),
                description: "End-to-end test: route winner through the Runtime executor."
                    .to_owned(),
                // No output_contract: NoopAgent emits free-form text,
                // and a structured-output contract would force the
                // proposal to be a JSON object. Validators stay no-op
                // without a contract.
                constraints: None,
                attributes: Some(attributes),
                external_context: None,
                metadata: None,
            }),
            execution_options: None,
        })
        .await
        .context("Orchestrate failed")?
        .into_inner();

    if response.task_id != "e2e-task-5" {
        bail!("response.task_id = {:?}", response.task_id);
    }
    if response.execution_id != "stub-invocation-1" {
        bail!(
            "execution_id should be the stub-runtime's canned value, got {:?}",
            response.execution_id
        );
    }
    let winner = response
        .winner
        .ok_or_else(|| anyhow!("Orchestrate returned no winner"))?;
    let winner_proposal = winner
        .proposal
        .ok_or_else(|| anyhow!("winner has no proposal"))?;
    if winner_proposal.content.trim().is_empty() {
        bail!("winner proposal content is empty");
    }
    info!(
        execution_id = response.execution_id.as_str(),
        winner_id = winner_proposal.proposal_id.as_str(),
        "Orchestrate routed through stub-runtime"
    );
    Ok(())
}
