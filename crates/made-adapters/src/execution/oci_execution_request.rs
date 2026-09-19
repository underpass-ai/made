use serde::{Deserialize, Serialize};

/// Command executed inside the image. No host executable or environment is inherited.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OciExecutionRequest {
    pub argv: Vec<String>,
}
