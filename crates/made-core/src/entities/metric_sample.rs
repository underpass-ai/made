use std::collections::BTreeMap;

use crate::value_objects::{MetricLabelName, MetricLabelValue, MetricName, MetricValue};

/// One flattened sample from a Prometheus exposition.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricSample {
    name: MetricName,
    labels: BTreeMap<MetricLabelName, MetricLabelValue>,
    value: MetricValue,
}

impl MetricSample {
    #[must_use]
    pub fn new(
        name: MetricName,
        labels: BTreeMap<MetricLabelName, MetricLabelValue>,
        value: MetricValue,
    ) -> Self {
        Self {
            name,
            labels,
            value,
        }
    }

    #[must_use]
    pub fn name(&self) -> &MetricName {
        &self.name
    }

    #[must_use]
    pub fn labels(&self) -> &BTreeMap<MetricLabelName, MetricLabelValue> {
        &self.labels
    }

    #[must_use]
    pub fn value(&self) -> MetricValue {
        self.value
    }
}
