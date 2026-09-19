use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub(crate) struct HttpWorkerConnectorConfig {
    pub(crate) base_url: String,
    pub(crate) operation_root: PathBuf,
    pub(crate) timeout: Duration,
}
