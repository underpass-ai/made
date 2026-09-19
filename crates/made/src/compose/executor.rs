//! Select the configured execution transport.
use crate::ComposeError;
use made_adapters::noop::NoopExecutor;
use made_adapters::runtime::{ExecutorBackendConfig, RuntimeExecutor};
use made_core::ports::ExecutorPort;
use std::sync::Arc;
pub(super) async fn wire() -> Result<Arc<dyn ExecutorPort>, ComposeError> {
    let executor: Arc<dyn ExecutorPort> = match ExecutorBackendConfig::from_env()? {
        ExecutorBackendConfig::Noop => Arc::new(NoopExecutor::new()),
        ExecutorBackendConfig::Runtime(config) => Arc::new(RuntimeExecutor::connect(config).await?),
    };
    Ok(executor)
}

pub(super) fn backend_name() -> &'static str {
    match ExecutorBackendConfig::from_env() {
        Ok(ExecutorBackendConfig::Runtime(_)) => "runtime",
        _ => "noop",
    }
}
