use made_app::usecases::{ProtocolSearchCursor, SearchProtocolInstancesInput};
use made_core::value_objects::{
    ProtocolIdPrefix, ProtocolInstancePageLimit, ProtocolLifecyclePhase,
};

use super::{
    domain_error_to_status, link_span_to_metadata, pb, unrehydratable_protocol_instance_state_from,
    GrpcResult, MadeGrpcService, Request, Response, Status,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.search_protocol_instances", skip_all)]
    pub(super) async fn handle_search_protocol_instances(
        &self,
        request: Request<pb::SearchProtocolInstancesRequest>,
    ) -> GrpcResult<pb::SearchProtocolInstancesResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let cursor = (!request.cursor.is_empty())
            .then(|| ProtocolSearchCursor::parse(request.cursor))
            .transpose()
            .map_err(domain_error_to_status)?;
        let limit = if request.limit == 0 {
            ProtocolInstancePageLimit::DEFAULT
        } else {
            ProtocolInstancePageLimit::new(
                u16::try_from(request.limit)
                    .map_err(|_| Status::invalid_argument("protocol search limit exceeds u16"))?,
            )
            .map_err(domain_error_to_status)?
        };
        let id_prefix = (!request.id_prefix.is_empty())
            .then(|| ProtocolIdPrefix::new(request.id_prefix))
            .transpose()
            .map_err(domain_error_to_status)?;
        let lifecycle = lifecycle_filter(request.lifecycle).map_err(Status::invalid_argument)?;
        let page = self
            .search_protocol_instances
            .execute(&SearchProtocolInstancesInput::new(
                cursor, limit, id_prefix, lifecycle,
            ))
            .await
            .map_err(domain_error_to_status)?;
        let mut instances = Vec::with_capacity(page.reads().len());
        for read in page.reads() {
            instances.push(
                match self
                    .resolve_protocol_definition
                    .execute(read.instance())
                    .await
                {
                    Ok(definition) => self
                        .render_read(read, &definition)
                        .map_err(domain_error_to_status)?,
                    Err(unreadable) => unrehydratable_protocol_instance_state_from(
                        read.instance().id(),
                        &unreadable.to_string(),
                    ),
                },
            );
        }
        Ok(Response::new(pb::SearchProtocolInstancesResponse {
            instances,
            next_cursor: page
                .next_cursor()
                .map_or_else(String::new, |cursor| cursor.as_str().to_owned()),
        }))
    }
}

fn lifecycle_filter(value: i32) -> Result<Option<ProtocolLifecyclePhase>, &'static str> {
    match pb::ProtocolLifecycleFilter::try_from(value)
        .map_err(|_| "unknown protocol lifecycle filter")?
    {
        pb::ProtocolLifecycleFilter::Unspecified => Ok(None),
        pb::ProtocolLifecycleFilter::Running => Ok(Some(ProtocolLifecyclePhase::Running)),
        pb::ProtocolLifecycleFilter::Paused => Ok(Some(ProtocolLifecyclePhase::Paused)),
        pb::ProtocolLifecycleFilter::Ended => Ok(Some(ProtocolLifecyclePhase::Ended)),
    }
}
