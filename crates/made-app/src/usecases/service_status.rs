use std::time::Duration;

use made_core::entities::Statistics;
use made_core::value_objects::RecorderName;

use super::ServiceHealth;

/// What an engine answers when it is asked how it is doing.
///
/// One shape for both compositions (ADR-014): the deployable service
/// renders it into `GetStatusResponse` and the in-process edition
/// renders it into the same tool JSON, so the two cannot drift on what
/// a status *is*.
///
/// `version` and `recorder` are compile-time facts of the engine that
/// answered — the crate it was built from and the metrics sink wired
/// into it — which is why they are `&'static str` rather than data read
/// from somewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceStatus {
    version: &'static str,
    uptime: Duration,
    health: ServiceHealth,
    recorder: RecorderName,
    statistics: Option<Statistics>,
}

impl ServiceStatus {
    #[must_use]
    pub const fn new(
        version: &'static str,
        uptime: Duration,
        health: ServiceHealth,
        recorder: RecorderName,
        statistics: Option<Statistics>,
    ) -> Self {
        Self {
            version,
            uptime,
            health,
            recorder,
            statistics,
        }
    }

    #[must_use]
    pub const fn version(&self) -> &'static str {
        self.version
    }

    /// How long this engine has been up. Whole seconds is what the
    /// contract carries, and the truncation happens here so the two
    /// renderings cannot round differently.
    #[must_use]
    pub const fn uptime_seconds(&self) -> u64 {
        self.uptime.as_secs()
    }

    #[must_use]
    pub const fn health(&self) -> ServiceHealth {
        self.health
    }

    /// What is recording operational metrics in this engine — `noop`,
    /// `prometheus`, or whatever a host injected.
    #[must_use]
    pub const fn recorder(&self) -> &'static str {
        self.recorder.as_str()
    }

    /// The counter snapshot, present only when the caller asked for it.
    #[must_use]
    pub const fn statistics(&self) -> Option<&Statistics> {
        self.statistics.as_ref()
    }
}
