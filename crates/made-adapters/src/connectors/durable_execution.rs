use std::fmt;

use made_core::value_objects::{
    ExecutionConnectorId, ExecutionIntent, ExecutionRecoveryCapability,
};

use super::{
    ConnectorError, ConnectorKind, DurableExecutionResult, ReceiptFilesystemConnector,
    ReceiptTransportConnector, RepositoryScriptConnector, SafeExecutionReceipt, ScriptInvocation,
    ScriptResolution,
};

/// Connects one repository/script executor to durable receipt storage and a
/// receipt transport.  Construction validates every advertised capability.
pub struct DurableExecutionConnector<R, F, T> {
    id: ExecutionConnectorId,
    recovery_capability: ExecutionRecoveryCapability,
    repository_script: R,
    filesystem: F,
    transport: T,
}

impl<R, F, T> fmt::Debug for DurableExecutionConnector<R, F, T>
where
    R: RepositoryScriptConnector,
    F: ReceiptFilesystemConnector,
    T: ReceiptTransportConnector,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableExecutionConnector")
            .field("id", &self.id)
            .field("recovery_capability", &self.recovery_capability)
            .field(
                "repository_script",
                &self.repository_script.descriptor().id(),
            )
            .field("filesystem", &self.filesystem.descriptor().id())
            .field("transport", &self.transport.descriptor().id())
            .finish_non_exhaustive()
    }
}

impl<R, F, T> DurableExecutionConnector<R, F, T>
where
    R: RepositoryScriptConnector,
    F: ReceiptFilesystemConnector,
    T: ReceiptTransportConnector,
{
    pub fn new(repository_script: R, filesystem: F, transport: T) -> Result<Self, ConnectorError> {
        let execution = repository_script.descriptor();
        execution.validate()?;
        execution.require_kind(ConnectorKind::RepositoryScript)?;
        filesystem.descriptor().validate()?;
        filesystem
            .descriptor()
            .require_kind(ConnectorKind::Filesystem)?;
        transport.descriptor().validate()?;
        transport
            .descriptor()
            .require_kind(ConnectorKind::Transport)?;

        Ok(Self {
            id: execution.id().clone(),
            recovery_capability: execution
                .recovery_capability()
                .expect("repository/script descriptor validation requires recovery"),
            repository_script,
            filesystem,
            transport,
        })
    }

    #[must_use]
    pub const fn connector_id(&self) -> &ExecutionConnectorId {
        &self.id
    }

    #[must_use]
    pub const fn recovery_capability(&self) -> ExecutionRecoveryCapability {
        self.recovery_capability
    }

    pub async fn execute_or_recover(
        &self,
        intent: &ExecutionIntent,
    ) -> Result<DurableExecutionResult, ConnectorError> {
        intent
            .validate()
            .map_err(|_| ConnectorError::InvalidIntent)?;
        if intent.connector_id() != &self.id
            || intent.recovery_capability() != self.recovery_capability
        {
            return Err(ConnectorError::ConnectorMismatch);
        }

        if let Some(receipt) = self
            .filesystem
            .load(intent.operation().operation_id())
            .await?
        {
            receipt.validate_for(intent, &self.id, self.recovery_capability)?;
            self.transport.publish(&receipt).await?;
            return Ok(DurableExecutionResult::Receipt(receipt));
        }

        let resolution = self
            .repository_script
            .execute_or_recover(ScriptInvocation::from_intent(intent))
            .await?;
        let observation = match resolution {
            ScriptResolution::ReconciliationRequired => {
                return Ok(DurableExecutionResult::ReconciliationRequired)
            }
            ScriptResolution::Observed(observation) => observation,
        };
        let receipt = SafeExecutionReceipt::from_observation(
            intent,
            self.id.clone(),
            self.recovery_capability,
            &observation,
        )?;
        self.filesystem.store(&receipt).await?;
        self.transport.publish(&receipt).await?;
        Ok(DurableExecutionResult::Receipt(receipt))
    }
}
