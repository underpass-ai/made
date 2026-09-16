use std::fmt;
use std::sync::Arc;

use made_core::entities::Statistics;
use made_core::error::DomainError;
use made_core::ports::StatisticsPort;

/// Reads the operational counters, whatever is keeping them.
///
/// The counters are the same aggregate the status answer carries when
/// a caller asks for it; this is the read on its own. One use case for
/// both compositions, so the numbers a client gets do not depend on
/// which engine it reached (ADR-014).
pub struct GetServiceMetricsUseCase {
    statistics: Arc<dyn StatisticsPort>,
}

impl fmt::Debug for GetServiceMetricsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("GetServiceMetricsUseCase").finish()
    }
}

impl GetServiceMetricsUseCase {
    #[must_use]
    pub fn new(statistics: Arc<dyn StatisticsPort>) -> Self {
        Self { statistics }
    }

    #[tracing::instrument(name = "get_service_metrics", skip_all)]
    pub async fn execute(&self) -> Result<Statistics, DomainError> {
        self.statistics.snapshot().await
    }
}

#[cfg(test)]
mod tests {
    use made_core::value_objects::{DurationMs, Specialty};

    use super::*;

    struct StatisticsFake(Statistics);

    #[async_trait::async_trait]
    impl StatisticsPort for StatisticsFake {
        async fn record_deliberation(
            &self,
            _: &Specialty,
            _: DurationMs,
        ) -> Result<(), DomainError> {
            Ok(())
        }

        async fn record_orchestration(&self, _: DurationMs) -> Result<(), DomainError> {
            Ok(())
        }

        async fn snapshot(&self) -> Result<Statistics, DomainError> {
            Ok(self.0.clone())
        }
    }

    #[tokio::test]
    async fn the_answer_is_the_snapshot_the_port_holds() {
        let mut snapshot = Statistics::new();
        snapshot.record_orchestration(DurationMs::from_millis(400));

        let read = GetServiceMetricsUseCase::new(Arc::new(StatisticsFake(snapshot.clone())))
            .execute()
            .await
            .unwrap();

        assert_eq!(read, snapshot);
        assert_eq!(read.total_orchestrations(), 1);
    }

    /// An engine that has done nothing answers zeros rather than
    /// nothing: an absent counter and a counter at zero are different
    /// claims, and only one of them is true here.
    #[tokio::test]
    async fn an_engine_that_has_done_nothing_answers_zeros() {
        let read = GetServiceMetricsUseCase::new(Arc::new(StatisticsFake(Statistics::new())))
            .execute()
            .await
            .unwrap();

        assert_eq!(read.total_deliberations(), 0);
        assert_eq!(read.total_orchestrations(), 0);
        assert_eq!(read.average_duration_ms(), 0.0);
        assert!(read.per_specialty().is_empty());
    }
}
