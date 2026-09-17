use made_mcp_proto::v1 as pb;

use crate::renderers::{MetricFamilyView, MetricSampleView};

use super::primitives::pb_value_to_json;

pub(crate) fn metric_family_view(family: pb::MetricFamily) -> MetricFamilyView {
    MetricFamilyView {
        name: family.name,
        help: family.help,
        kind: family.r#type,
        samples: family.samples.into_iter().map(metric_sample_view).collect(),
    }
}

fn metric_sample_view(sample: pb::MetricSample) -> MetricSampleView {
    MetricSampleView {
        name: sample.name,
        labels: sample.labels.into_iter().collect(),
        value: sample
            .value
            .map_or(serde_json::Value::Null, pb_value_to_json),
    }
}
