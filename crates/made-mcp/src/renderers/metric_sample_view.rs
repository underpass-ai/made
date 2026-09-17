use std::collections::BTreeMap;

use serde_json::{json, Value};

use made_core::value_objects::FiniteMetricValue;

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
    pub(crate) fn finite_value(value: FiniteMetricValue) -> Value {
        const EXACT_WHOLE_LIMIT: f64 = 9_007_199_254_740_992.0;
        let value = value.get();
        if value.is_finite() && value.fract() == 0.0 && value.abs() <= EXACT_WHOLE_LIMIT {
            #[allow(clippy::cast_possible_truncation)]
            let whole = value as i64;
            return Value::Number(whole.into());
        }
        Value::Number(
            serde_json::Number::from_f64(value)
                .expect("FiniteMetricValue always contains a JSON number"),
        )
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
    use made_core::value_objects::MetricValue;

    use super::*;

    #[test]
    fn whole_prometheus_values_use_the_transport_stable_json_shape() {
        let MetricValue::Finite(whole) = MetricValue::from_f64(1.0) else {
            unreachable!()
        };
        let MetricValue::Finite(fraction) = MetricValue::from_f64(1.25) else {
            unreachable!()
        };
        assert_eq!(MetricSampleView::finite_value(whole), json!(1));
        assert_eq!(MetricSampleView::finite_value(fraction), json!(1.25));
    }
}
