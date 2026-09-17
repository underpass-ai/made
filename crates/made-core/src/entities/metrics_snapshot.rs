use crate::entities::MetricFamily;
use crate::value_objects::PrometheusText;

/// One read of an in-process operational metrics registry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetricsSnapshot {
    text: PrometheusText,
    families: Vec<MetricFamily>,
}

impl MetricsSnapshot {
    #[must_use]
    pub fn new(text: PrometheusText, families: Vec<MetricFamily>) -> Self {
        Self { text, families }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn text(&self) -> &PrometheusText {
        &self.text
    }

    #[must_use]
    pub fn families(&self) -> &[MetricFamily] {
        &self.families
    }
}
