use serde::{Deserialize, Serialize};

/// Exact authorized repository change, sealed inside the semantic operation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitExecutionRequest {
    pub repository: String,
    pub branch: String,
    pub expected_tip: String,
    pub diff: String,
    pub diff_sha256: String,
}
