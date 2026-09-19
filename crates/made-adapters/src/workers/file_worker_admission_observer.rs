use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use fs2::FileExt;
use made_app::workers::{CeremonyWorkerAdmissionDecision, CeremonyWorkerAdmissionObserver};
use made_core::DomainError;

#[derive(Debug)]
pub struct FileWorkerAdmissionObserver {
    file: Mutex<File>,
}

impl FileWorkerAdmissionObserver {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io_error)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(io_error)?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }

    fn append(&self, decision: &CeremonyWorkerAdmissionDecision) -> Result<(), DomainError> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| DomainError::InvariantViolated {
                reason: "worker admission journal lock is poisoned",
            })?;
        file.lock_exclusive().map_err(io_error)?;
        let result = serde_json::to_writer(
            &mut *file,
            &serde_json::json!({
                "sequence": decision.sequence(),
                "policy_version": decision.policy_version().value(),
                "root_id": decision.request().root_id().as_str(),
                "operation_id": decision.request().operation_id().as_str(),
                "priority": decision.request().priority().value(),
                "weight": decision.request().weight().value(),
                "cost": decision.request().cost().value(),
                "requested_capacity": decision.request().requested_capacity().value(),
                "reason": decision.reason().to_string(),
            }),
        )
        .map_err(admission_storage_error)
        .and_then(|()| file.write_all(b"\n").map_err(io_error))
        .and_then(|()| file.sync_data().map_err(io_error));
        let unlock = FileExt::unlock(&*file).map_err(io_error);
        result.and(unlock)
    }
}

impl CeremonyWorkerAdmissionObserver for FileWorkerAdmissionObserver {
    fn observe(&self, decision: &CeremonyWorkerAdmissionDecision) {
        if let Err(error) = self.append(decision) {
            tracing::error!(%error, "worker admission decision could not be persisted");
        }
    }
}

fn io_error(error: std::io::Error) -> DomainError {
    admission_storage_error(error)
}

fn admission_storage_error<T>(_error: T) -> DomainError {
    DomainError::InvariantViolated {
        reason: "worker admission journal is unavailable",
    }
}
