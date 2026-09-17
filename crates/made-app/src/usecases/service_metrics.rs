use made_core::entities::{MetricsSnapshot, Statistics};

/// Durable service counters and the current in-process registry read at one
/// use-case boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ServiceMetrics {
    statistics: Statistics,
    registry: MetricsSnapshot,
}

impl ServiceMetrics {
    #[must_use]
    pub fn new(statistics: Statistics, registry: MetricsSnapshot) -> Self {
        Self {
            statistics,
            registry,
        }
    }

    #[must_use]
    pub fn statistics(&self) -> &Statistics {
        &self.statistics
    }

    #[must_use]
    pub fn registry(&self) -> &MetricsSnapshot {
        &self.registry
    }
}
