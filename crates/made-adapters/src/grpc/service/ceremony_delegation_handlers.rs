//! Claim, do the work, complete — the two ends of a step the host
//! runs itself.
//!
//! These two are the only ceremony RPCs with a gap between them that
//! the engine does not fill. Claiming takes the lease durably before
//! the host starts, so a second host asking for the same step is
//! refused rather than left to discover the collision afterwards;
//! completing writes down the result the host reports and infers
//! nothing about it.
//!
//! Both answer with the session, like every other move.

use super::{
    claim_ceremony_step_input_from_proto, complete_ceremony_step_input_from_proto,
    domain_error_to_status, link_span_to_metadata, pb, CeremonyId, GrpcResult, MadeGrpcService,
    Request, Response,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.claim_ceremony_step", skip_all)]
    pub(super) async fn handle_claim_ceremony_step(
        &self,
        request: Request<pb::ClaimCeremonyStepRequest>,
    ) -> GrpcResult<pb::ClaimCeremonyStepResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let ceremony_id =
            CeremonyId::new(request.ceremony_id.clone()).map_err(domain_error_to_status)?;
        let (instance, definition) = self.session(&ceremony_id).await?;
        let input = claim_ceremony_step_input_from_proto(request, &definition, &instance)
            .map_err(domain_error_to_status)?;
        self.claim_ceremony_step
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        // Read back rather than render what was loaded: claiming
        // answers with the attempt it took, and the session a caller
        // gets must already show the step as claimed. The definition
        // is the one resolved above — a claim changes the instance and
        // never the definition.
        let claimed = self
            .get_ceremony_instance
            .execute(&ceremony_id)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ClaimCeremonyStepResponse {
            instance: Some(Self::render(&claimed, &definition).map_err(domain_error_to_status)?),
        }))
    }

    #[tracing::instrument(name = "rpc.complete_ceremony_step", skip_all)]
    pub(super) async fn handle_complete_ceremony_step(
        &self,
        request: Request<pb::CompleteCeremonyStepRequest>,
    ) -> GrpcResult<pb::CompleteCeremonyStepResponse> {
        link_span_to_metadata(&request);
        let input = complete_ceremony_step_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .complete_ceremony_step
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::CompleteCeremonyStepResponse {
            instance: Some(state),
        }))
    }
}
