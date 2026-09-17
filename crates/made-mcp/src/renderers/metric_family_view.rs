use serde_json::{json, Value};

use super::MetricSampleView;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MetricFamilyView {
    pub(crate) name: String,
    pub(crate) help: String,
    pub(crate) kind: String,
    pub(crate) samples: Vec<MetricSampleView>,
}

impl MetricFamilyView {
    #[must_use]
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "help": self.help,
            "type": self.kind,
            "samples": self.samples.iter().map(MetricSampleView::to_json).collect::<Vec<_>>(),
        })
    }
}
