use crate::entities::MetricsSnapshot;
use crate::error::DomainError;

/// Reads the same in-process registry an operational metrics recorder writes.
pub trait MetricsSnapshotPort: Send + Sync {
    fn snapshot(&self) -> Result<MetricsSnapshot, DomainError>;
}
