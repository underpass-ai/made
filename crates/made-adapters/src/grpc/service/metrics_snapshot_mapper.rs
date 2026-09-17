use made_core::entities::{MetricFamily, MetricSample};
use made_core::value_objects::MetricValue;
use made_proto::v1 as pb;
use prost_types::{value::Kind, Value};

pub(super) fn metric_family_to_proto(family: &MetricFamily) -> pb::MetricFamily {
    pb::MetricFamily {
        name: family.name().as_str().to_owned(),
        help: family.help().as_str().to_owned(),
        r#type: family.kind().as_str().to_owned(),
        samples: family
            .samples()
            .iter()
            .map(metric_sample_to_proto)
            .collect(),
    }
}

fn metric_sample_to_proto(sample: &MetricSample) -> pb::MetricSample {
    let labels = sample
        .labels()
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), value.as_str().to_owned()))
        .collect();
    pb::MetricSample {
        name: sample.name().as_str().to_owned(),
        labels,
        value: Some(metric_value_to_proto(sample.value())),
    }
}

fn metric_value_to_proto(value: MetricValue) -> Value {
    let kind = match value {
        MetricValue::Finite(value) => Kind::NumberValue(value.get()),
        MetricValue::Nan => Kind::StringValue("NaN".to_owned()),
        MetricValue::PositiveInfinity => Kind::StringValue("+Inf".to_owned()),
        MetricValue::NegativeInfinity => Kind::StringValue("-Inf".to_owned()),
    };
    Value { kind: Some(kind) }
}
