use made_app::usecases::{CeremonySearchCursor, SearchCeremonyInstancesInput};
use made_core::value_objects::{
    CeremonyIdPrefix, CeremonyInstancePageLimit, CeremonyLifecyclePhase,
};

use super::{
    domain_error_to_status, link_span_to_metadata, pb, unrehydratable_ceremony_instance_state_from,
    GrpcResult, MadeGrpcService, Request, Response, Status,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.search_ceremony_instances", skip_all)]
    pub(super) async fn handle_search_ceremony_instances(
        &self,
        request: Request<pb::SearchCeremonyInstancesRequest>,
    ) -> GrpcResult<pb::SearchCeremonyInstancesResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let cursor = (!request.cursor.is_empty())
            .then(|| CeremonySearchCursor::parse(request.cursor))
            .transpose()
            .map_err(domain_error_to_status)?;
        let limit = if request.limit == 0 {
            CeremonyInstancePageLimit::DEFAULT
        } else {
            CeremonyInstancePageLimit::new(
                u16::try_from(request.limit)
                    .map_err(|_| Status::invalid_argument("ceremony search limit exceeds u16"))?,
            )
            .map_err(domain_error_to_status)?
        };
        let id_prefix = (!request.id_prefix.is_empty())
            .then(|| CeremonyIdPrefix::new(request.id_prefix))
            .transpose()
            .map_err(domain_error_to_status)?;
        let lifecycle = lifecycle_filter(request.lifecycle).map_err(Status::invalid_argument)?;
        let page = self
            .search_ceremony_instances
            .execute(&SearchCeremonyInstancesInput::new(
                cursor, limit, id_prefix, lifecycle,
            ))
            .await
            .map_err(domain_error_to_status)?;
        let mut instances = Vec::with_capacity(page.reads().len());
        for read in page.reads() {
            instances.push(
                match self
                    .resolve_ceremony_definition
                    .execute(read.instance())
                    .await
                {
                    Ok(definition) => self
                        .render_read(read, &definition)
                        .map_err(domain_error_to_status)?,
                    Err(unreadable) => unrehydratable_ceremony_instance_state_from(
                        read.instance().id(),
                        &unreadable.to_string(),
                    ),
                },
            );
        }
        Ok(Response::new(pb::SearchCeremonyInstancesResponse {
            instances,
            next_cursor: page
                .next_cursor()
                .map_or_else(String::new, |cursor| cursor.as_str().to_owned()),
        }))
    }
}

fn lifecycle_filter(value: i32) -> Result<Option<CeremonyLifecyclePhase>, &'static str> {
    match pb::CeremonyLifecycleFilter::try_from(value)
        .map_err(|_| "unknown ceremony lifecycle filter")?
    {
        pb::CeremonyLifecycleFilter::Unspecified => Ok(None),
        pb::CeremonyLifecycleFilter::Running => Ok(Some(CeremonyLifecyclePhase::Running)),
        pb::CeremonyLifecycleFilter::Paused => Ok(Some(CeremonyLifecyclePhase::Paused)),
        pb::CeremonyLifecycleFilter::Ended => Ok(Some(CeremonyLifecyclePhase::Ended)),
    }
}
