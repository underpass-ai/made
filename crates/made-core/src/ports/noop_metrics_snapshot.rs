use crate::entities::MetricsSnapshot;
use crate::error::DomainError;
use crate::ports::MetricsSnapshotPort;

/// Registry reader for a host that intentionally has no metrics registry.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopMetricsSnapshot;

impl MetricsSnapshotPort for NoopMetricsSnapshot {
    fn snapshot(&self) -> Result<MetricsSnapshot, DomainError> {
        Ok(MetricsSnapshot::empty())
    }
}
