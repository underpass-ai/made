use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{CeremonyEventTransportPort, MetricsSnapshotPort, PositionedRecord};

use crate::ceremony_event_wire::CeremonyEventWire;

/// Append-only JSON Lines sink for embedded ceremony-event publication.
pub struct JsonLinesCeremonyEventSink {
    writer: Mutex<BufWriter<File>>,
    metrics: Option<Arc<dyn MetricsSnapshotPort>>,
}

impl JsonLinesCeremonyEventSink {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DomainError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|_| DomainError::InvariantViolated {
                reason: "ceremony event sink file could not be opened",
            })?;
        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
            metrics: None,
        })
    }

    /// Open an event sink that follows every event line with a snapshot of
    /// the same in-process registry the host records into.
    pub fn open_with_metrics(
        path: impl AsRef<Path>,
        metrics: Arc<dyn MetricsSnapshotPort>,
    ) -> Result<Self, DomainError> {
        let mut sink = Self::open(path)?;
        sink.metrics = Some(metrics);
        Ok(sink)
    }
}

impl std::fmt::Debug for JsonLinesCeremonyEventSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JsonLinesCeremonyEventSink")
            .field("metrics_wired", &self.metrics.is_some())
            .finish()
    }
}

#[async_trait]
impl CeremonyEventTransportPort for JsonLinesCeremonyEventSink {
    async fn deliver(&self, record: &PositionedRecord) -> Result<(), DomainError> {
        let encoded = serde_json::to_vec(&CeremonyEventWire::from(record)).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "ceremony event sink record could not be encoded",
            }
        })?;
        let metrics = self
            .metrics
            .as_ref()
            .map(|metrics| {
                metrics.snapshot().and_then(|snapshot| {
                    serde_json::to_vec(&super::metrics_snapshot_json::metrics_snapshot_json(
                        &snapshot,
                    ))
                    .map_err(|_| DomainError::InvariantViolated {
                        reason: "metrics snapshot could not be encoded",
                    })
                })
            })
            .transpose()?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| DomainError::InvariantViolated {
                reason: "ceremony event sink lock is poisoned",
            })?;
        writer
            .write_all(&encoded)
            .and_then(|()| writer.write_all(b"\n"))
            .and_then(|()| {
                if let Some(metrics) = &metrics {
                    writer.write_all(metrics)?;
                    writer.write_all(b"\n")?;
                }
                Ok(())
            })
            .and_then(|()| writer.flush())
            .map_err(|_| DomainError::InvariantViolated {
                reason: "ceremony event sink record could not be appended",
            })
    }
}
