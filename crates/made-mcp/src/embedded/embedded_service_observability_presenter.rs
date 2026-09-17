use made_app::usecases::ServiceStatus;
use made_core::entities::Statistics;
use serde_json::{json, Value};

use crate::renderers::StatisticsView;

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
pub(super) fn present_service_metrics(statistics: &Statistics) -> Value {
    StatisticsView::envelope(Some(&statistics_view(statistics)))
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
    use std::time::Duration;

    use made_app::usecases::ServiceHealth;
    use made_core::value_objects::{DurationMs, Specialty};

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
        assert_eq!(
            present_service_metrics(&Statistics::new()),
            json!({
                "stats": {
                    "total_deliberations": 0,
                    "total_orchestrations": 0,
                    "total_duration_ms": 0,
                    "average_duration_ms": 0.0,
                    "per_specialty_counts": {},
                }
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

        let rendered = present_service_metrics(&counters);

        assert!(
            rendered["stats"]["average_duration_ms"].is_f64(),
            "{rendered}"
        );
    }
}
