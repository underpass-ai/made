use tonic::transport::Channel;

use super::RuntimeExecutorConfig;

/// Runtime-backed executor.
#[derive(Debug, Clone)]
pub struct RuntimeExecutor {
    pub(super) channel: Channel,
    pub(super) config: RuntimeExecutorConfig,
}
