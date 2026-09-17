use std::collections::BTreeMap;

use made_core::entities::{MetricFamily, MetricSample, MetricsSnapshot};
use made_core::error::DomainError;
use made_core::value_objects::{
    MetricHelp, MetricKind, MetricLabelName, MetricLabelValue, MetricName, MetricValue,
    PrometheusText,
};
use prometheus::proto::{Metric, MetricFamily as PrometheusFamily, MetricType};
use prometheus::{Encoder, Registry, TextEncoder};

pub(super) fn snapshot(registry: &Registry) -> Result<MetricsSnapshot, DomainError> {
    let gathered = registry.gather();
    let mut buffer = Vec::new();
    TextEncoder::new()
        .encode(&gathered, &mut buffer)
        .map_err(|error| {
            tracing::error!(%error, "prometheus metrics encode failed");
            DomainError::InvariantViolated {
                reason: "prometheus metrics could not be encoded",
            }
        })?;
    let text = String::from_utf8(buffer).map_err(|error| {
        tracing::error!(%error, "prometheus metrics were not UTF-8");
        DomainError::InvariantViolated {
            reason: "prometheus metrics were not UTF-8",
        }
    })?;
    let families = gathered
        .iter()
        .map(family_snapshot)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MetricsSnapshot::new(PrometheusText::new(text), families))
}

fn family_snapshot(family: &PrometheusFamily) -> Result<MetricFamily, DomainError> {
    let name = MetricName::new(family.get_name())?;
    let kind = metric_kind(family.get_field_type());
    let mut samples = Vec::new();
    for metric in family.get_metric() {
        flatten_metric(&name, kind, metric, &mut samples)?;
    }
    Ok(MetricFamily::new(
        name,
        MetricHelp::new(family.get_help()),
        kind,
        samples,
    ))
}

fn metric_kind(kind: MetricType) -> MetricKind {
    match kind {
        MetricType::COUNTER => MetricKind::Counter,
        MetricType::GAUGE => MetricKind::Gauge,
        MetricType::HISTOGRAM => MetricKind::Histogram,
        MetricType::SUMMARY => MetricKind::Summary,
        MetricType::UNTYPED => MetricKind::Untyped,
    }
}

#[allow(deprecated)]
fn flatten_metric(
    name: &MetricName,
    kind: MetricKind,
    metric: &Metric,
    samples: &mut Vec<MetricSample>,
) -> Result<(), DomainError> {
    let labels = labels(metric)?;
    match kind {
        MetricKind::Counter => push_sample(
            samples,
            name.clone(),
            labels,
            metric.get_counter().get_value(),
        ),
        MetricKind::Gauge => push_sample(
            samples,
            name.clone(),
            labels,
            metric.get_gauge().get_value(),
        ),
        MetricKind::Untyped => push_sample(
            samples,
            name.clone(),
            labels,
            metric.get_untyped().get_value(),
        ),
        MetricKind::Histogram => flatten_histogram(name, metric, labels, samples)?,
        MetricKind::Summary => flatten_summary(name, metric, labels, samples)?,
    }
    Ok(())
}

#[allow(deprecated)]
fn flatten_histogram(
    name: &MetricName,
    metric: &Metric,
    labels: BTreeMap<MetricLabelName, MetricLabelValue>,
    samples: &mut Vec<MetricSample>,
) -> Result<(), DomainError> {
    let histogram = metric.get_histogram();
    let bucket_name = suffix_name(name, "_bucket")?;
    let le = MetricLabelName::new("le")?;
    let mut positive_infinity_seen = false;
    for bucket in histogram.get_bucket() {
        let upper = bucket.get_upper_bound();
        positive_infinity_seen |= upper.is_infinite() && upper.is_sign_positive();
        let mut bucket_labels = labels.clone();
        bucket_labels.insert(le.clone(), MetricLabelValue::new(prometheus_number(upper)));
        push_sample(
            samples,
            bucket_name.clone(),
            bucket_labels,
            bucket.get_cumulative_count() as f64,
        );
    }
    if !positive_infinity_seen {
        let mut bucket_labels = labels.clone();
        bucket_labels.insert(le, MetricLabelValue::new("+Inf"));
        push_sample(
            samples,
            bucket_name,
            bucket_labels,
            histogram.get_sample_count() as f64,
        );
    }
    push_sample(
        samples,
        suffix_name(name, "_sum")?,
        labels.clone(),
        histogram.get_sample_sum(),
    );
    push_sample(
        samples,
        suffix_name(name, "_count")?,
        labels,
        histogram.get_sample_count() as f64,
    );
    Ok(())
}

fn flatten_summary(
    name: &MetricName,
    metric: &Metric,
    labels: BTreeMap<MetricLabelName, MetricLabelValue>,
    samples: &mut Vec<MetricSample>,
) -> Result<(), DomainError> {
    let summary = metric.get_summary();
    let quantile_name = MetricLabelName::new("quantile")?;
    for quantile in summary.get_quantile() {
        let mut quantile_labels = labels.clone();
        quantile_labels.insert(
            quantile_name.clone(),
            MetricLabelValue::new(prometheus_number(quantile.get_quantile())),
        );
        push_sample(samples, name.clone(), quantile_labels, quantile.get_value());
    }
    push_sample(
        samples,
        suffix_name(name, "_sum")?,
        labels.clone(),
        summary.get_sample_sum(),
    );
    push_sample(
        samples,
        suffix_name(name, "_count")?,
        labels,
        summary.get_sample_count() as f64,
    );
    Ok(())
}

fn labels(metric: &Metric) -> Result<BTreeMap<MetricLabelName, MetricLabelValue>, DomainError> {
    metric
        .get_label()
        .iter()
        .map(|label| {
            Ok((
                MetricLabelName::new(label.get_name())?,
                MetricLabelValue::new(label.get_value()),
            ))
        })
        .collect()
}

fn suffix_name(name: &MetricName, suffix: &str) -> Result<MetricName, DomainError> {
    MetricName::new(format!("{}{suffix}", name.as_str()))
}

fn push_sample(
    samples: &mut Vec<MetricSample>,
    name: MetricName,
    labels: BTreeMap<MetricLabelName, MetricLabelValue>,
    value: f64,
) {
    samples.push(MetricSample::new(
        name,
        labels,
        MetricValue::from_f64(value),
    ));
}

fn prometheus_number(value: f64) -> String {
    if value == f64::INFINITY {
        "+Inf".to_owned()
    } else if value == f64::NEG_INFINITY {
        "-Inf".to_owned()
    } else if value.is_nan() {
        "NaN".to_owned()
    } else {
        value.to_string()
    }
}
