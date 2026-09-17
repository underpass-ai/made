use crate::entities::MetricSample;
use crate::value_objects::{MetricHelp, MetricKind, MetricName};

/// One Prometheus metric family and its flattened samples.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricFamily {
    name: MetricName,
    help: MetricHelp,
    kind: MetricKind,
    samples: Vec<MetricSample>,
}

impl MetricFamily {
    #[must_use]
    pub fn new(
        name: MetricName,
        help: MetricHelp,
        kind: MetricKind,
        samples: Vec<MetricSample>,
    ) -> Self {
        Self {
            name,
            help,
            kind,
            samples,
        }
    }

    #[must_use]
    pub fn name(&self) -> &MetricName {
        &self.name
    }

    #[must_use]
    pub fn help(&self) -> &MetricHelp {
        &self.help
    }

    #[must_use]
    pub fn kind(&self) -> MetricKind {
        self.kind
    }

    #[must_use]
    pub fn samples(&self) -> &[MetricSample] {
        &self.samples
    }
}
