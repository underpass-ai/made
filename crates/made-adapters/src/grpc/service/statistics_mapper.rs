use made_app::usecases::ServiceStatus;
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

/// Map the shared [`ServiceStatus`] into the contract's status message.
///
/// `recorder` has no field on the wire yet: the proto carries four,
/// and a fifth is G3's to add once the deployable edition answers with
/// its registry rather than the five legacy counters. Dropping it here
/// keeps the two MCP arms byte-identical instead of giving one of them
/// a key the other cannot fill.
pub(super) fn service_status_to_proto(status: &ServiceStatus) -> pb::GetStatusResponse {
    pb::GetStatusResponse {
        version: status.version().to_owned(),
        uptime_seconds: status.uptime_seconds(),
        health: status.health().as_str().to_owned(),
        stats: status.statistics().map(statistics_to_proto),
    }
}
