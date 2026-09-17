use made_app::usecases::{ServiceMetrics, ServiceStatus};
use made_core::entities::{MetricFamily, MetricSample, Statistics};
use made_core::value_objects::{FiniteMetricValue, MetricValue};
use serde_json::{json, Value};

use crate::renderers::{MetricFamilyView, MetricSampleView, ServiceMetricsView, StatisticsView};

/// Render a status the way the contract renders it.
///
/// The four keys `GetStatusResponse` carries, in its order, and no
/// fifth: an in-process engine that answered with a key the deployable
/// one cannot fill would be a divergence in the direction this
/// workstream exists to remove. `ServiceStatus::recorder` is therefore
/// read by a host through the facade and is not on either arm's wire —
/// G3 puts the registry itself on both.
pub(super) fn present_service_status(status: &ServiceStatus) -> Value {
    let statistics = status.statistics().map(statistics_view);
    json!({
        "version": status.version(),
        "uptime_seconds": status.uptime_seconds(),
        "health": status.health().as_str(),
        "stats": statistics.as_ref().map_or(Value::Null, StatisticsView::to_json),
    })
}

/// Render the counter snapshot the way the contract renders it.
pub(super) fn present_service_metrics(metrics: &ServiceMetrics) -> Value {
    ServiceMetricsView {
        statistics: Some(statistics_view(metrics.statistics())),
        registry_text: metrics.registry().text().as_str().to_owned(),
        registry: metrics
            .registry()
            .families()
            .iter()
            .map(metric_family_view)
            .collect(),
    }
    .to_json()
}

fn metric_family_view(family: &MetricFamily) -> MetricFamilyView {
    MetricFamilyView {
        name: family.name().as_str().to_owned(),
        help: family.help().as_str().to_owned(),
        kind: family.kind().as_str().to_owned(),
        samples: family.samples().iter().map(metric_sample_view).collect(),
    }
}

fn metric_sample_view(sample: &MetricSample) -> MetricSampleView {
    let labels = sample
        .labels()
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), value.as_str().to_owned()))
        .collect();
    let value = match sample.value() {
        MetricValue::Finite(value) => finite_metric_value(value),
        MetricValue::Nan => Value::String("NaN".to_owned()),
        MetricValue::PositiveInfinity => Value::String("+Inf".to_owned()),
        MetricValue::NegativeInfinity => Value::String("-Inf".to_owned()),
    };
    MetricSampleView {
        name: sample.name().as_str().to_owned(),
        labels,
        value,
    }
}

/// Render a Prometheus double with the same JSON number shape after a
/// protobuf round trip. Whole values in the exact integer range stay
/// integers on both editions.
fn finite_metric_value(value: FiniteMetricValue) -> Value {
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

/// The five counters `Statistics` carries on the wire.
///
/// Feeds the same shared view as the gRPC and fixture backends, including
/// `average_duration_ms` staying a fraction.
fn statistics_view(statistics: &Statistics) -> StatisticsView {
    let per_specialty_counts = statistics
        .per_specialty()
        .iter()
        .map(|(specialty, count)| (specialty.as_str().to_owned(), *count))
        .collect();
    StatisticsView {
        total_deliberations: statistics.total_deliberations(),
        total_orchestrations: statistics.total_orchestrations(),
        total_duration_ms: statistics.total_duration().get(),
        average_duration_ms: statistics.average_duration_ms(),
        per_specialty_counts,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use made_app::usecases::ServiceHealth;
    use made_core::entities::{MetricFamily, MetricSample, MetricsSnapshot};
    use made_core::value_objects::{
        DurationMs, MetricHelp, MetricKind, MetricName, PrometheusText, Specialty,
    };

    use super::*;

    fn status(statistics: Option<Statistics>) -> ServiceStatus {
        ServiceStatus::new(
            "9.9.9",
            Duration::from_secs(61),
            ServiceHealth::Healthy,
            made_core::value_objects::RecorderName::new("prometheus"),
            statistics,
        )
    }

    #[test]
    fn a_status_answers_with_the_four_keys_the_contract_carries() {
        let rendered = present_service_status(&status(None));

        assert_eq!(
            rendered,
            json!({
                "version": "9.9.9",
                "uptime_seconds": 61,
                "health": "healthy",
                "stats": Value::Null,
            })
        );
        // The recorder a host reads through the facade is not on the
        // wire: the deployable arm has no field for it.
        assert!(rendered.get("recorder").is_none());
    }

    #[test]
    fn the_counters_are_rendered_only_when_the_caller_asked_for_them() {
        let mut counters = Statistics::new();
        counters.record_deliberation(
            &Specialty::new("facilitation").unwrap(),
            DurationMs::from_millis(1_500),
        );

        let rendered = present_service_status(&status(Some(counters)));

        assert_eq!(
            rendered["stats"],
            json!({
                "total_deliberations": 1,
                "total_orchestrations": 0,
                "total_duration_ms": 1_500,
                "average_duration_ms": 1_500.0,
                "per_specialty_counts": { "facilitation": 1 },
            })
        );
    }

    /// An engine that has done nothing answers zeros under the same
    /// five keys, not an empty object: the tool reports the same
    /// families whichever edition served it.
    #[test]
    fn metrics_answer_with_every_family_even_at_zero() {
        let metrics = ServiceMetrics::new(Statistics::new(), MetricsSnapshot::empty());
        assert_eq!(
            present_service_metrics(&metrics),
            json!({
                "stats": {
                    "total_deliberations": 0,
                    "total_orchestrations": 0,
                    "total_duration_ms": 0,
                    "average_duration_ms": 0.0,
                    "per_specialty_counts": {},
                },
                "registry_text": "",
                "registry": [],
            })
        );
    }

    /// The average is a fraction on both arms, because a `Struct`
    /// carries it as a double over the wire and a JSON number that
    /// lost its decimal point is a different type.
    #[test]
    fn the_average_stays_a_fraction() {
        let mut counters = Statistics::new();
        counters.record_orchestration(DurationMs::from_millis(1_000));
        let metrics = ServiceMetrics::new(counters, MetricsSnapshot::empty());

        let rendered = present_service_metrics(&metrics);

        assert!(
            rendered["stats"]["average_duration_ms"].is_f64(),
            "{rendered}"
        );
    }

    #[test]
    fn metric_projection_preserves_finite_and_non_finite_values() {
        let values = [
            MetricValue::from_f64(2.0),
            MetricValue::from_f64(2.5),
            MetricValue::from_f64(f64::NAN),
            MetricValue::from_f64(f64::INFINITY),
            MetricValue::from_f64(f64::NEG_INFINITY),
        ];
        let samples = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                MetricSample::new(
                    MetricName::new(format!("made_projection_{index}")).expect("valid metric name"),
                    BTreeMap::new(),
                    value,
                )
            })
            .collect();
        let snapshot = MetricsSnapshot::new(
            PrometheusText::new(String::new()),
            vec![MetricFamily::new(
                MetricName::new("made_projection").expect("valid metric name"),
                MetricHelp::new("projection values"),
                MetricKind::Gauge,
                samples,
            )],
        );

        let rendered = present_service_metrics(&ServiceMetrics::new(Statistics::new(), snapshot));
        let samples = rendered["registry"][0]["samples"]
            .as_array()
            .expect("samples array");

        assert_eq!(samples[0]["value"], json!(2));
        assert_eq!(samples[1]["value"], json!(2.5));
        assert_eq!(samples[2]["value"], json!("NaN"));
        assert_eq!(samples[3]["value"], json!("+Inf"));
        assert_eq!(samples[4]["value"], json!("-Inf"));
        assert!(samples.iter().all(|sample| !sample["value"].is_null()));
    }
    #[test]
    fn whole_prometheus_values_use_the_transport_stable_json_shape() {
        let MetricValue::Finite(whole) = MetricValue::from_f64(1.0) else {
            unreachable!()
        };
        let MetricValue::Finite(fraction) = MetricValue::from_f64(1.25) else {
            unreachable!()
        };
        assert_eq!(finite_metric_value(whole), json!(1));
        assert_eq!(finite_metric_value(fraction), json!(1.25));
    }
}
