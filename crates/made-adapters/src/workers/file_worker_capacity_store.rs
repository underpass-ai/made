use super::worker_capacity_reservation::WorkerCapacityReservation;
use async_trait::async_trait;
use fs2::FileExt;
use made_app::services::SessionStream;
use made_app::workers::{
    WorkerCapacityLimits, WorkerCapacityPort, WorkerCapacityRenewalGuard, WorkerCapacityRequest,
};
use made_core::error::DomainError;
use made_core::value_objects::{ExecutionOperationId, LeaseOwnerId, StepClaimFence};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use time::OffsetDateTime;

/// Process-shared local admission ledger. All workers must use the same directory.
/// The journal determines whether a bound reservation is still authoritative.
/// Filesystem locks are acquired on blocking threads; no Tokio runtime is blocked
/// waiting for another process. Errors keep reservations rather than free capacity.
pub struct FileWorkerCapacityStore {
    directory: PathBuf,
    limits: WorkerCapacityLimits,
    stream: Arc<SessionStream>,
}

impl std::fmt::Debug for FileWorkerCapacityStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileWorkerCapacityStore")
            .field("directory", &self.directory)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl FileWorkerCapacityStore {
    pub fn new(
        directory: impl AsRef<Path>,
        limits: WorkerCapacityLimits,
        stream: Arc<SessionStream>,
    ) -> Result<Self, DomainError> {
        std::fs::create_dir_all(directory.as_ref()).map_err(capacity_storage_error)?;
        Ok(Self {
            directory: directory.as_ref().to_owned(),
            limits,
            stream,
        })
    }

    async fn lock(&self) -> Result<File, DomainError> {
        let path = self.directory.join("capacity.lock");
        tokio::task::spawn_blocking(move || {
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(path)
                .map_err(capacity_storage_error)?;
            file.lock_exclusive().map_err(capacity_storage_error)?;
            Ok(file)
        })
        .await
        .map_err(|_| DomainError::InvariantViolated {
            reason: "worker capacity lock task failed",
        })?
    }

    fn read(&self) -> Result<Vec<WorkerCapacityReservation>, DomainError> {
        match std::fs::read(self.directory.join("capacity.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(capacity_storage_error),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(capacity_storage_error(error)),
        }
    }

    fn write(&self, reservations: &[WorkerCapacityReservation]) -> Result<(), DomainError> {
        let bytes = serde_json::to_vec(reservations).map_err(capacity_storage_error)?;
        let path = self.directory.join("capacity.next");
        let mut file = File::create(&path).map_err(capacity_storage_error)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(capacity_storage_error)?;
        std::fs::rename(path, self.directory.join("capacity.json"))
            .map_err(capacity_storage_error)?;
        File::open(&self.directory)
            .and_then(|file| file.sync_all())
            .map_err(capacity_storage_error)
    }

    async fn reconcile(
        &self,
        reservations: Vec<WorkerCapacityReservation>,
        now: OffsetDateTime,
    ) -> Result<Vec<WorkerCapacityReservation>, DomainError> {
        let mut retained = Vec::new();
        for mut reservation in reservations {
            let request = &reservation.request;
            if reservation.fence.is_none() {
                if request.pending_until > now {
                    retained.push(reservation);
                }
                continue;
            }
            let session = self.stream.load(&request.ceremony_id).await?;
            let record = session.instance.step_record(&request.step_id);
            let current = session.instance.step_claim_fence(&request.step_id).ok();
            let operation_matches = record.is_some_and(|record| {
                ExecutionOperationId::for_step(
                    &request.ceremony_id,
                    &request.step_id,
                    record.state_visit(),
                    record.state_iteration(),
                    record.iteration(),
                ) == request.operation_id
            });
            let owner_matches = record
                .and_then(|record| record.lease())
                .is_some_and(|lease| lease.owner_id() == &request.owner_id);
            let live = !session.instance.is_ended()
                && operation_matches
                && owner_matches
                && record.is_some_and(|record| record.has_live_lease_at(now));
            if live && (reservation.fence.is_none() || reservation.fence == current) {
                // Crash between journal claim and ledger binding: recover the binding.
                reservation.fence = current;
                retained.push(reservation);
            }
        }
        Ok(retained)
    }
}

fn capacity_storage_error<T>(_error: T) -> DomainError {
    DomainError::InvariantViolated {
        reason: "worker capacity storage is unavailable",
    }
}

#[async_trait]
impl WorkerCapacityPort for FileWorkerCapacityStore {
    async fn reserve(
        &self,
        request: &WorkerCapacityRequest,
        now: OffsetDateTime,
    ) -> Result<bool, DomainError> {
        let _lock = self.lock().await?;
        let mut rows = self.reconcile(self.read()?, now).await?;
        if let Some(existing) = rows
            .iter()
            .find(|row| row.request.operation_id == request.operation_id)
        {
            let same = existing.request.owner_id == request.owner_id
                && existing.request.root_id == request.root_id
                && existing.request.connector_id == request.connector_id
                && existing.request.provider_id == request.provider_id;
            self.write(&rows)?;
            return Ok(same);
        }
        let global = rows.len();
        let root = rows
            .iter()
            .filter(|row| row.request.root_id == request.root_id)
            .count();
        let connector = rows
            .iter()
            .filter(|row| row.request.connector_id == request.connector_id)
            .count();
        let provider = rows
            .iter()
            .filter(|row| row.request.provider_id == request.provider_id)
            .count();
        let admitted = global < self.limits.global.value() as usize
            && root < self.limits.per_root.value() as usize
            && connector < self.limits.per_connector.value() as usize
            && provider < self.limits.per_provider.value() as usize;
        if admitted {
            rows.push(WorkerCapacityReservation {
                request: request.clone(),
                fence: None,
            });
        }
        self.write(&rows)?;
        Ok(admitted)
    }

    async fn bind(
        &self,
        operation: &ExecutionOperationId,
        owner: &LeaseOwnerId,
        fence: &StepClaimFence,
    ) -> Result<(), DomainError> {
        let _lock = self.lock().await?;
        let mut rows = self.read()?;
        let row = rows
            .iter_mut()
            .find(|row| &row.request.operation_id == operation && &row.request.owner_id == owner)
            .ok_or(DomainError::Conflict {
                what: "worker_capacity_reservation",
            })?;
        if row.fence.as_ref().is_some_and(|existing| existing != fence) {
            return Err(DomainError::Conflict {
                what: "worker_capacity_fence",
            });
        }
        row.fence = Some(fence.clone());
        self.write(&rows)
    }

    async fn lock_renewal(
        &self,
        operation: &ExecutionOperationId,
        owner: &LeaseOwnerId,
    ) -> Result<Box<dyn WorkerCapacityRenewalGuard>, DomainError> {
        let lock = self.lock().await?;
        let rows = self.read()?;
        let reserved = rows
            .iter()
            .any(|row| &row.request.operation_id == operation && &row.request.owner_id == owner);
        if !reserved {
            return Err(DomainError::Conflict {
                what: "worker_capacity_reservation",
            });
        }
        Ok(Box::new(lock))
    }

    async fn release(
        &self,
        operation: &ExecutionOperationId,
        owner: &LeaseOwnerId,
    ) -> Result<(), DomainError> {
        let _lock = self.lock().await?;
        let mut rows = self.read()?;
        rows.retain(|row| &row.request.operation_id != operation || &row.request.owner_id != owner);
        self.write(&rows)
    }
}
