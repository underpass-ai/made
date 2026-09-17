use serde_json::{json, Value};

use super::{MetricFamilyView, StatisticsView};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ServiceMetricsView {
    pub(crate) statistics: Option<StatisticsView>,
    pub(crate) registry_text: String,
    pub(crate) registry: Vec<MetricFamilyView>,
}

impl ServiceMetricsView {
    #[must_use]
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "stats": self.statistics.as_ref().map_or(Value::Null, StatisticsView::to_json),
            "registry_text": self.registry_text,
            "registry": self.registry.iter().map(MetricFamilyView::to_json).collect::<Vec<_>>(),
        })
    }
}
