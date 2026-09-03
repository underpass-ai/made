use super::RuntimeExecutorConfig;

/// Binary-level execution backend selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutorBackendConfig {
    Noop,
    Runtime(RuntimeExecutorConfig),
}
