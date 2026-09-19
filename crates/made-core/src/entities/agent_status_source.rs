use serde::{Deserialize, Serialize};

/// Provenance of the observation shown to a reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatusSource {
    HostReport,
    DerivedLease,
    HostDiscoveryUnsupported,
}
