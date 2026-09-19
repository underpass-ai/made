use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct GitWorkerConnectorConfig {
    pub(crate) repository: PathBuf,
    pub(crate) scratch: PathBuf,
}
