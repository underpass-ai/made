use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    MetricsRecorderPort, PositionedRecord, ProtocolEventStorePort, ProtocolEventSubscriberPort,
};
use made_core::value_objects::{ProtocolEventPageLimit, ProtocolId, StreamVersion};
use tokio::sync::Mutex;

use super::protocol_fanout_projection::ProtocolFanoutProjection;

/// Projects ordered sealed history even when append notifications arrive out of order.
pub struct ProtocolFanoutMetricsSubscriber {
    events: Arc<dyn ProtocolEventStorePort>,
    metrics: Arc<dyn MetricsRecorderPort>,
    streams: Mutex<BTreeMap<ProtocolId, ProtocolFanoutProjection>>,
}

impl ProtocolFanoutMetricsSubscriber {
    #[must_use]
    pub fn new(
        events: Arc<dyn ProtocolEventStorePort>,
        metrics: Arc<dyn MetricsRecorderPort>,
    ) -> Self {
        Self {
            events,
            metrics,
            streams: Mutex::new(BTreeMap::new()),
        }
    }

    async fn catch_up(&self, id: &ProtocolId, through: StreamVersion) -> Result<(), DomainError> {
        let mut streams = self.streams.lock().await;
        let projection = streams.entry(id.clone()).or_default();
        while projection.version < through {
            let records = self
                .events
                .read(id, projection.version, ProtocolEventPageLimit::DEFAULT)
                .await?;
            let before = projection.version;
            for record in records
                .iter()
                .take_while(|record| StreamVersion::from_sequence(record.sequence()) <= through)
            {
                projection.observe(record, self.metrics.as_ref())?;
            }
            if projection.version == before {
                return Err(DomainError::InvariantViolated {
                    reason: "fanout metric stream read made no progress",
                });
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for ProtocolFanoutMetricsSubscriber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProtocolFanoutMetricsSubscriber")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ProtocolEventSubscriberPort for ProtocolFanoutMetricsSubscriber {
    async fn observe(&self, records: &[PositionedRecord]) {
        let mut cuts = BTreeMap::new();
        for positioned in records {
            let record = &positioned.record;
            let version = StreamVersion::from_sequence(record.sequence());
            cuts.entry(record.protocol_id())
                .and_modify(|cut: &mut StreamVersion| *cut = (*cut).max(version))
                .or_insert(version);
        }
        for (id, through) in cuts {
            if let Err(error) = self.catch_up(id, through).await {
                tracing::warn!(protocol_id = %id, %error, "fanout metrics projection failed; next notification retries its frontier");
            }
        }
    }
}
