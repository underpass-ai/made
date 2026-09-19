use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
    LocalExecutionMode, LocalExecutionOutcome, LocalNetworkPolicy, LocalProcessTermination,
};

/// Safe receipt containing counts and policy, never process output or secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalExecutionReceipt {
    pub mode: LocalExecutionMode,
    pub filesystem_root: PathBuf,
    pub network: LocalNetworkPolicy,
    pub timeout_ms: u64,
    pub max_output_bytes: u64,
    pub request_digest: String,
    pub outcome: LocalExecutionOutcome,
    pub exit_code: Option<i32>,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub output_bytes: u64,
    pub process_termination: LocalProcessTermination,
}
