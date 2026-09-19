use made_core::value_objects::{ExecutionIntent, ExternalOperationId, StepClaimFence, StepResult};
use made_core::{ports::CeremonyExecutionObservation, DomainError};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use time::OffsetDateTime;

/// Host-owned durable receipt. Kept outside writable execution workspaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OperationRecord {
    pub digest: String,
    pub fence: StepClaimFence,
    pub external_id: String,
    pub result: StepResult,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

impl OperationRecord {
    pub fn observe(
        self,
        intent: &ExecutionIntent,
    ) -> Result<CeremonyExecutionObservation, DomainError> {
        if self.digest != intent.operation().request_digest().as_str() {
            return Err(DomainError::Conflict {
                what: "operation_request_digest",
            });
        }
        if &self.fence != intent.claim_fence() {
            return Err(DomainError::Conflict {
                what: "operation_producer_claim_fence",
            });
        }
        Ok(CeremonyExecutionObservation::new(
            self.fence,
            Some(ExternalOperationId::new(self.external_id)?),
            self.result,
            Vec::new(),
            self.observed_at,
        ))
    }
    pub fn load(path: &Path) -> Result<Option<Self>, DomainError> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| invalid("operation record corrupt")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(invalid("operation record unavailable")),
        }
    }
    pub fn persist(&self, path: &Path) -> Result<(), DomainError> {
        seal(
            path,
            &serde_json::to_vec(self).map_err(|_| invalid("operation encoding failed"))?,
        )
        .map(|_| ())
    }
}

pub(crate) fn invalid(reason: &'static str) -> DomainError {
    DomainError::InvariantViolated { reason }
}

pub(crate) fn seal(path: &Path, bytes: &[u8]) -> Result<bool, DomainError> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("operation path has no parent"))?;
    let temp = parent.join(format!(".operation-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::hard_link(&temp, path)?;
        File::open(parent)?.sync_all()
    })();
    let _ = std::fs::remove_file(temp);
    match result {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if std::fs::read(path).map_err(|_| invalid("sealed operation unreadable"))? != bytes {
                return Err(DomainError::Conflict {
                    what: "sealed_operation",
                });
            }
            Ok(false)
        }
        Err(_) => Err(invalid("operation cannot be sealed")),
    }
}

pub(crate) fn key(intent: &ExecutionIntent) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(intent.operation().operation_id().as_str().as_bytes())
    )
}
