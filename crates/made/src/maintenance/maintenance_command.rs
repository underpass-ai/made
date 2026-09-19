use made_core::ports::ArtifactIdempotencyKey;
use made_core::value_objects::AuthorizationAction;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Local administrative intent; filesystem destinations are explicit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum MaintenanceCommand {
    Backup {
        destination: PathBuf,
        key: ArtifactIdempotencyKey,
    },
    Restore {
        source: PathBuf,
        destination: PathBuf,
    },
    RestorePostgres {
        source: PathBuf,
        target_url_env: String,
    },
    ReconcileExecution {
        receipt: Box<made_core::value_objects::ExecutionReceipt>,
    },
    GcPreview {
        retired_before: String,
    },
    GcApply {
        plan: made_adapters::artifacts::ArtifactGcPlan,
    },
    ReleaseProtection {
        key: ArtifactIdempotencyKey,
        reason: String,
    },
}

impl MaintenanceCommand {
    pub const fn action(&self) -> AuthorizationAction {
        match self {
            Self::Backup { .. } => AuthorizationAction::BackupStore,
            Self::Restore { .. } | Self::RestorePostgres { .. } => {
                AuthorizationAction::RestoreStore
            }
            Self::ReconcileExecution { .. } => AuthorizationAction::ReconcileExecutionOperation,
            Self::GcPreview { .. } | Self::GcApply { .. } => {
                AuthorizationAction::GarbageCollectArtifacts
            }
            Self::ReleaseProtection { .. } => AuthorizationAction::ReleaseArtifactProtection,
        }
    }
}
