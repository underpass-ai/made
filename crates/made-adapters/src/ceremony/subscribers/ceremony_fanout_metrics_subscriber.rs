use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyEventStorePort, CeremonyEventSubscriberPort, MetricsRecorderPort, PositionedRecord,
};
use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, StreamVersion};
use tokio::sync::Mutex;

use super::ceremony_fanout_projection::CeremonyFanoutProjection;

/// Projects ordered sealed history even when append notifications arrive out of order.
pub struct CeremonyFanoutMetricsSubscriber {
    events: Arc<dyn CeremonyEventStorePort>,
    metrics: Arc<dyn MetricsRecorderPort>,
    streams: Mutex<BTreeMap<CeremonyId, CeremonyFanoutProjection>>,
}

impl CeremonyFanoutMetricsSubscriber {
    #[must_use]
    pub fn new(
        events: Arc<dyn CeremonyEventStorePort>,
        metrics: Arc<dyn MetricsRecorderPort>,
    ) -> Self {
        Self {
            events,
            metrics,
            streams: Mutex::new(BTreeMap::new()),
        }
    }

    async fn catch_up(&self, id: &CeremonyId, through: StreamVersion) -> Result<(), DomainError> {
        let mut streams = self.streams.lock().await;
        let projection = streams.entry(id.clone()).or_default();
        while projection.version < through {
            let records = self
                .events
                .read(id, projection.version, CeremonyEventPageLimit::DEFAULT)
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

impl std::fmt::Debug for CeremonyFanoutMetricsSubscriber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonyFanoutMetricsSubscriber")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl CeremonyEventSubscriberPort for CeremonyFanoutMetricsSubscriber {
    async fn observe(&self, records: &[PositionedRecord]) {
        let mut cuts = BTreeMap::new();
        for positioned in records {
            let record = &positioned.record;
            let version = StreamVersion::from_sequence(record.sequence());
            cuts.entry(record.ceremony_id())
                .and_modify(|cut: &mut StreamVersion| *cut = (*cut).max(version))
                .or_insert(version);
        }
        for (id, through) in cuts {
            if let Err(error) = self.catch_up(id, through).await {
                tracing::warn!(ceremony_id = %id, %error, "fanout metrics projection failed; next notification retries its frontier");
            }
        }
    }
}
