use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use made_core::error::DomainError;
use made_core::ports::{MetricsRecorderPort, StatisticsPort};

use super::{ServiceHealth, ServiceStatus};

/// Reports how the engine that answers is doing.
///
/// The composition it runs in is the only thing that differs: the
/// deployable service builds it from the ports it wired at boot, the
/// in-process edition from the ports the host handed its builder. What
/// a status *is* — the version, how long this engine has been up, its
/// condition, what is recording, and the counters when the caller asks
/// for them — is decided here and nowhere else, so an adapter cannot
/// answer one thing over the wire and another in process (ADR-014).
pub struct GetServiceStatusUseCase {
    statistics: Arc<dyn StatisticsPort>,
    metrics: Arc<dyn MetricsRecorderPort>,
    version: &'static str,
    /// When this engine started. A monotonic instant rather than a
    /// wall clock: uptime must not move when the host's clock does.
    started_at: Instant,
}

impl fmt::Debug for GetServiceStatusUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GetServiceStatusUseCase")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl GetServiceStatusUseCase {
    #[must_use]
    pub fn new(
        statistics: Arc<dyn StatisticsPort>,
        metrics: Arc<dyn MetricsRecorderPort>,
        version: &'static str,
        started_at: Instant,
    ) -> Self {
        Self {
            statistics,
            metrics,
            version,
            started_at,
        }
    }

    /// `include_statistics` is the caller's, not the engine's: reading
    /// the counters can reach a store, and a health check that only
    /// wants to know whether the engine is up should not pay for it.
    #[tracing::instrument(name = "get_service_status", skip_all)]
    pub async fn execute(&self, include_statistics: bool) -> Result<ServiceStatus, DomainError> {
        let statistics = if include_statistics {
            Some(self.statistics.snapshot().await?)
        } else {
            None
        };
        Ok(ServiceStatus::new(
            self.version,
            self.started_at.elapsed(),
            // Nothing computes a condition yet, on either edition, and
            // both have always answered `healthy`. One place says so
            // now, so the day something does compute it, both editions
            // get the answer at once.
            ServiceHealth::Healthy,
            self.metrics.recorder_name(),
            statistics,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use made_core::entities::Statistics;
    use made_core::ports::NoopMetricsRecorder;
    use made_core::value_objects::{DurationMs, Specialty};

    use super::*;

    #[derive(Default)]
    struct StatisticsFake {
        snapshot: Statistics,
    }

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
            Ok(self.snapshot.clone())
        }
    }

    struct FailingStatistics;

    #[async_trait::async_trait]
    impl StatisticsPort for FailingStatistics {
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
            Err(DomainError::InvariantViolated {
                reason: "the counter store is not answering",
            })
        }
    }

    fn use_case(statistics: Arc<dyn StatisticsPort>) -> GetServiceStatusUseCase {
        GetServiceStatusUseCase::new(
            statistics,
            Arc::new(NoopMetricsRecorder),
            "0.0.0-test",
            Instant::now()
                .checked_sub(Duration::from_secs(90))
                .expect("ninety seconds ago is a representable instant"),
        )
    }

    #[tokio::test]
    async fn a_status_names_the_engine_its_uptime_and_what_is_recording() {
        let status = use_case(Arc::new(StatisticsFake::default()))
            .execute(false)
            .await
            .unwrap();

        assert_eq!(status.version(), "0.0.0-test");
        assert!(status.uptime_seconds() >= 90);
        assert_eq!(status.health(), ServiceHealth::Healthy);
        // The one question an operator asks of an edition whose
        // recorder is the host's choice.
        assert_eq!(status.recorder(), "noop");
    }

    /// Not asking for the counters must not read them: the port is
    /// never touched, which is what makes a cheap health check cheap.
    #[tokio::test]
    async fn the_counters_are_absent_unless_the_caller_asks_and_fatal_when_they_fail() {
        let omitted = use_case(Arc::new(FailingStatistics)).execute(false).await;
        assert!(omitted.unwrap().statistics().is_none());

        let asked = use_case(Arc::new(FailingStatistics)).execute(true).await;
        assert!(matches!(asked, Err(DomainError::InvariantViolated { .. })));
    }

    #[tokio::test]
    async fn the_counters_are_the_snapshot_the_port_answers_with() {
        let mut snapshot = Statistics::new();
        snapshot.record_deliberation(
            &Specialty::new("facilitation").unwrap(),
            DurationMs::from_millis(1_200),
        );

        let status = use_case(Arc::new(StatisticsFake {
            snapshot: snapshot.clone(),
        }))
        .execute(true)
        .await
        .unwrap();

        assert_eq!(status.statistics(), Some(&snapshot));
    }
}
