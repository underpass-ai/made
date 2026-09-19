use std::path::Path;
use std::sync::Arc;

use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::services::SessionStream;
use made_app::usecases::VerifyCeremonyJournalUseCase;
use made_app::workers::InspectExecutionRecoveryUseCase;
use made_core::ports::ExecutionReceiptStorePort;
use made_core::value_objects::{ExecutionRecoveryPageLimit, StepStatus};

/// Re-read persisted truth; counters alone cannot prove a soak completed its work.
pub async fn verify(
    store: &Arc<SqliteCeremonyStore>,
    stream: &SessionStream,
    inspector: &InspectExecutionRecoveryUseCase,
    operation_root: &Path,
    ceremony_prefix: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    let limit = ExecutionRecoveryPageLimit::new(ExecutionRecoveryPageLimit::MAX)?;
    let verifier = VerifyCeremonyJournalUseCase::new(store.clone());
    let mut cursor = None;
    let mut completed = 0;
    loop {
        let page = store.recoverable(cursor.as_ref(), limit).await?;
        let (operations, next) = page.into_parts();
        for operation in operations {
            if !operation
                .ceremony_id()
                .as_str()
                .starts_with(ceremony_prefix)
            {
                continue;
            }
            let session = stream.load(operation.ceremony_id()).await?;
            if session
                .instance
                .step_record(operation.step_id())
                .map(made_core::value_objects::StepExecutionRecord::status)
                != Some(StepStatus::Completed)
                || session
                    .instance
                    .execution_receipt_link(operation.operation_id())
                    .is_none()
                || !verifier.execute(operation.ceremony_id()).await?.is_intact()
            {
                return Err(format!(
                    "uncompleted or corrupt ceremony {}",
                    operation.ceremony_id()
                )
                .into());
            }
            let receipt = store
                .receipt(operation.operation_id())
                .await?
                .ok_or("missing receipt")?;
            receipt.validate()?;
            let operation_id = operation.operation_id().as_str();
            let request = std::fs::read(operation_root.join(format!("{operation_id}.request")))?;
            let effect = std::fs::read(
                operation_root
                    .join("effects")
                    .join(format!("{operation_id}.json")),
            )?;
            if request != effect
                || operation.request().as_bytes() != request
                || receipt.request_digest() != operation.request_digest()
                || !receipt.result().status().is_success()
            {
                return Err(
                    format!("external effect or receipt differs for {operation_id}").into(),
                );
            }
            completed += 1;
        }
        cursor = next;
        if cursor.is_none() {
            break;
        }
    }
    cursor = None;
    loop {
        let page = inspector.execute(cursor.as_ref(), limit).await?;
        if !page.items().is_empty() {
            return Err("soak left unapplied execution operations".into());
        }
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            break;
        }
    }
    Ok(completed)
}
