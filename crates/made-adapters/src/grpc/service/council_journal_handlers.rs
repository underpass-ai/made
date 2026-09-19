use super::{
    domain_error_to_status, link_span_to_metadata, pb, GrpcResult, MadeGrpcService, Request,
    Response,
};
use crate::grpc::mappers::council_journal::{lease_from_proto, lease_to_proto, record_to_proto};
use made_core::value_objects::{
    CouncilJournalConsumer, CouncilJournalPageLimit, CouncilJournalPosition, DurationMs,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.read_council_events", skip_all)]
    pub(super) async fn handle_read_council_events(
        &self,
        request: Request<pb::ReadCouncilEventsRequest>,
    ) -> GrpcResult<pb::ReadCouncilEventsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let after = request
            .after
            .map(CouncilJournalPosition::new)
            .transpose()
            .map_err(domain_error_to_status)?;
        let limit = if request.limit == 0 {
            CouncilJournalPageLimit::default()
        } else {
            CouncilJournalPageLimit::new(
                usize::try_from(request.limit)
                    .map_err(|_| tonic::Status::invalid_argument("limit exceeds host size"))?,
            )
            .map_err(domain_error_to_status)?
        };
        let records = self
            .council_journal
            .read(after, limit)
            .await
            .map_err(domain_error_to_status)?;
        let next_after = records
            .last()
            .map(made_core::entities::CouncilJournalRecord::position)
            .or(after)
            .map(CouncilJournalPosition::value);
        let records = records
            .iter()
            .map(record_to_proto)
            .collect::<Result<Vec<_>, _>>()
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ReadCouncilEventsResponse {
            records,
            next_after,
        }))
    }
    #[tracing::instrument(name = "rpc.get_council_event_cursor", skip_all)]
    pub(super) async fn handle_get_council_event_cursor(
        &self,
        request: Request<pb::GetCouncilEventCursorRequest>,
    ) -> GrpcResult<pb::GetCouncilEventCursorResponse> {
        link_span_to_metadata(&request);
        let consumer = CouncilJournalConsumer::new(request.into_inner().consumer)
            .map_err(domain_error_to_status)?;
        let acknowledged_through = self
            .council_journal
            .position(&consumer)
            .await
            .map_err(domain_error_to_status)?
            .map(CouncilJournalPosition::value);
        Ok(Response::new(pb::GetCouncilEventCursorResponse {
            acknowledged_through,
        }))
    }
    #[tracing::instrument(name = "rpc.lease_council_events", skip_all)]
    pub(super) async fn handle_lease_council_events(
        &self,
        request: Request<pb::LeaseCouncilEventsRequest>,
    ) -> GrpcResult<pb::LeaseCouncilEventsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let consumer =
            CouncilJournalConsumer::new(request.consumer).map_err(domain_error_to_status)?;
        let lease = self
            .council_journal
            .lease(&consumer, DurationMs::from_millis(request.duration_ms))
            .await
            .map_err(domain_error_to_status)?;
        let lease = lease
            .as_ref()
            .map(lease_to_proto)
            .transpose()
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::LeaseCouncilEventsResponse { lease }))
    }
    #[tracing::instrument(name = "rpc.acknowledge_council_events", skip_all)]
    pub(super) async fn handle_acknowledge_council_events(
        &self,
        request: Request<pb::AcknowledgeCouncilEventsRequest>,
    ) -> GrpcResult<pb::AcknowledgeCouncilEventsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let lease = lease_from_proto(request.lease).map_err(domain_error_to_status)?;
        let through =
            CouncilJournalPosition::new(request.through).map_err(domain_error_to_status)?;
        self.council_journal
            .acknowledge(&lease, through)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::AcknowledgeCouncilEventsResponse {}))
    }
    #[tracing::instrument(name = "rpc.release_council_events", skip_all)]
    pub(super) async fn handle_release_council_events(
        &self,
        request: Request<pb::ReleaseCouncilEventsRequest>,
    ) -> GrpcResult<pb::ReleaseCouncilEventsResponse> {
        link_span_to_metadata(&request);
        let lease = lease_from_proto(request.into_inner().lease).map_err(domain_error_to_status)?;
        self.council_journal
            .release(&lease)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ReleaseCouncilEventsResponse {}))
    }
}
