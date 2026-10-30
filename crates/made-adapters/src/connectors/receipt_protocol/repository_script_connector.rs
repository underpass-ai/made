use super::{ScriptInvocation, ScriptResolution};
use crate::connectors::{ConnectorDescriptor, ConnectorError};
use async_trait::async_trait;

#[async_trait]
pub trait RepositoryScriptConnector: Send + Sync {
    fn descriptor(&self) -> &ConnectorDescriptor;
    async fn execute_or_recover(
        &self,
        invocation: ScriptInvocation,
    ) -> Result<ScriptResolution, ConnectorError>;
}
