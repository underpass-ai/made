use std::collections::BTreeMap;

use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MetricSampleView {
    pub(crate) name: String,
    pub(crate) labels: BTreeMap<String, String>,
    pub(crate) value: Value,
}

impl MetricSampleView {
    /// Render a Prometheus double with the same JSON number shape after a
    /// protobuf round trip. Whole values in the exact integer range stay
    /// integers on both editions.
    pub(crate) fn finite_value(value: f64) -> Value {
        const EXACT_WHOLE_LIMIT: f64 = 9_007_199_254_740_992.0;
        if value.is_finite() && value.fract() == 0.0 && value.abs() <= EXACT_WHOLE_LIMIT {
            #[allow(clippy::cast_possible_truncation)]
            let whole = value as i64;
            return Value::Number(whole.into());
        }
        serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
    }

    #[must_use]
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "labels": self.labels,
            "value": self.value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_prometheus_values_use_the_transport_stable_json_shape() {
        assert_eq!(MetricSampleView::finite_value(1.0), json!(1));
        assert_eq!(MetricSampleView::finite_value(1.25), json!(1.25));
    }
}
