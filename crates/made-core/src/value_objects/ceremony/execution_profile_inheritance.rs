use serde::{Deserialize, Serialize};

/// Closed provenance categories for inherited host profile values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionProfileInheritance {
    RoleDefault,
    StepDefault,
    CeremonyDefault,
    HostDefault,
    Checkpoint,
}
