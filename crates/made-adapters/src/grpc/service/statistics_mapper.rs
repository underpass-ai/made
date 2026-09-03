use made_core::entities::Statistics;
use made_proto::v1 as pb;

/// Map the domain [`made_core::entities::Statistics`] into the
/// protobuf `Statistics` message. Kept here, next to the only call
/// sites, because it is a pure transport concern.
pub(super) fn statistics_to_proto(stats: &Statistics) -> pb::Statistics {
    let per_specialty_counts = stats
        .per_specialty()
        .iter()
        .map(|(sp, count)| (sp.as_str().to_owned(), *count))
        .collect();
    pb::Statistics {
        total_deliberations: stats.total_deliberations(),
        total_orchestrations: stats.total_orchestrations(),
        total_duration_ms: stats.total_duration().get(),
        average_duration_ms: stats.average_duration_ms(),
        per_specialty_counts,
    }
}
