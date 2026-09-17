use std::collections::BTreeMap;

use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MetricSampleView {
    pub(crate) name: String,
    pub(crate) labels: BTreeMap<String, String>,
    pub(crate) value: Value,
}

impl MetricSampleView {
    #[must_use]
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "labels": self.labels,
            "value": self.value,
        })
    }
}
