use made_core::entities::{MetricFamily, MetricSample, MetricsSnapshot};
use made_core::value_objects::MetricValue;
use serde_json::{json, Value};

pub(super) fn metrics_snapshot_json(snapshot: &MetricsSnapshot) -> Value {
    json!({
        "record_type": "metrics_snapshot",
        "registry_text": snapshot.text().as_str(),
        "registry": snapshot.families().iter().map(metric_family_json).collect::<Vec<_>>(),
    })
}

fn metric_family_json(family: &MetricFamily) -> Value {
    json!({
        "name": family.name().as_str(),
        "help": family.help().as_str(),
        "type": family.kind().as_str(),
        "samples": family.samples().iter().map(metric_sample_json).collect::<Vec<_>>(),
    })
}

fn metric_sample_json(sample: &MetricSample) -> Value {
    let labels = sample
        .labels()
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), value.as_str().to_owned()))
        .collect::<std::collections::BTreeMap<_, _>>();
    json!({
        "name": sample.name().as_str(),
        "labels": labels,
        "value": metric_value_json(sample.value()),
    })
}

fn metric_value_json(value: MetricValue) -> Value {
    match value {
        MetricValue::Finite(value) => {
            serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
        }
        MetricValue::Nan => Value::String("NaN".to_owned()),
        MetricValue::PositiveInfinity => Value::String("+Inf".to_owned()),
        MetricValue::NegativeInfinity => Value::String("-Inf".to_owned()),
    }
}
