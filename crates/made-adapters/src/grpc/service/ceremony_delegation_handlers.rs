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

use made_app::budgets::{BudgetedStepClaimInput, BudgetedStepClaimOutput};
use made_app::usecases::{CeremonyInstanceRead, ReadCeremonyEventsInput, StartCeremonyStepOutput};
use made_core::entities::CeremonyDefinition;
use made_core::error::DomainError;
use made_core::value_objects::{CeremonyEventPageLimit, StreamVersion, TraceId};

use super::{
    budget_error_to_status, budget_reservation_estimate_from_proto,
    claim_ceremony_step_input_from_proto, complete_ceremony_step_input_from_proto,
    domain_error_to_status, link_span_to_metadata, pb, CeremonyId, GrpcResult, MadeGrpcService,
    Request, Response, Status,
};

impl MadeGrpcService {
    pub(super) async fn handle_renew_ceremony_step_lease(
        &self,
        request: Request<pb::RenewCeremonyStepLeaseRequest>,
    ) -> GrpcResult<pb::RenewCeremonyStepLeaseResponse> {
        use made_core::value_objects::{
            DurationMs, IdempotencyKey, LeaseOwnerId, StepClaimFence, StepId,
            StepLeaseRenewalRequest,
        };
        let request = request.into_inner();
        let input = made_app::workers::RenewCeremonyStepLeaseInput {
            ceremony_id: CeremonyId::new(request.ceremony_id).map_err(domain_error_to_status)?,
            step_id: StepId::new(request.step_id).map_err(domain_error_to_status)?,
            claim_fence: StepClaimFence::new(request.claim_fence)
                .map_err(domain_error_to_status)?,
            owner: LeaseOwnerId::new(request.lease_owner_id).map_err(domain_error_to_status)?,
            request: StepLeaseRenewalRequest {
                id: IdempotencyKey::new(request.renewal_id).map_err(domain_error_to_status)?,
                ttl: DurationMs::from_millis(request.lease_ttl_ms),
            },
        };
        let receipt = self
            .renew_ceremony_step_lease
            .as_ref()
            .ok_or_else(|| Status::unimplemented("delegated lease renewal is not configured"))?
            .execute_request(input)
            .await
            .map_err(domain_error_to_status)?;
        let moment = |at: time::OffsetDateTime| {
            at.format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default()
        };
        Ok(Response::new(pb::RenewCeremonyStepLeaseResponse {
            renewal_id: receipt
                .request
                .expect("public renewal request")
                .id
                .as_str()
                .to_owned(),
            claim_fence: receipt.claim_fence.as_str().to_owned(),
            effective_lease_expires_at: moment(receipt.expires_at),
            renewed_at: moment(receipt.renewed_at),
        }))
    }
    #[tracing::instrument(name = "rpc.claim_ceremony_step", skip_all)]
    pub(super) async fn handle_claim_ceremony_step(
        &self,
        request: Request<pb::ClaimCeremonyStepRequest>,
    ) -> GrpcResult<pb::ClaimCeremonyStepResponse> {
        link_span_to_metadata(&request);
        let mut request = request.into_inner();
        let ceremony_id =
            CeremonyId::new(request.ceremony_id.clone()).map_err(domain_error_to_status)?;
        let (instance, definition) = self.session(&ceremony_id).await?;
        let reservation = request.budget_reservation.take();
        let input = claim_ceremony_step_input_from_proto(request, &definition, &instance)
            .map_err(domain_error_to_status)?;
        if instance.budget_account_id().is_some() {
            let reservation = reservation.ok_or_else(|| {
                Status::failed_precondition(
                    "budgeted ceremony claims require a reservation estimate",
                )
            })?;
            let reservation = budget_reservation_estimate_from_proto(reservation)
                .map_err(domain_error_to_status)?;
            let output = self
                .budgeted_step_claim
                .as_deref()
                .ok_or_else(|| {
                    Status::failed_precondition("budgeted step admission is not configured")
                })?
                .execute(BudgetedStepClaimInput::new(input, reservation))
                .await
                .map_err(budget_error_to_status)?;
            let budget = Some(budget_claim_admission(&output));
            self.claim_response(output.claim(), budget, &definition)
                .await
        } else {
            if reservation.is_some() {
                return Err(Status::invalid_argument(
                    "budget reservation supplied for an unbudgeted ceremony",
                ));
            }
            let output = self
                .claim_ceremony_step
                .execute(input)
                .await
                .map_err(domain_error_to_status)?;
            self.claim_response(&output, None, &definition).await
        }
    }

    async fn claim_response(
        &self,
        claim: &StartCeremonyStepOutput,
        budget: Option<pb::BudgetClaimAdmission>,
        definition: &CeremonyDefinition,
    ) -> GrpcResult<pb::ClaimCeremonyStepResponse> {
        Ok(Response::new(pb::ClaimCeremonyStepResponse {
            instance: Some(self.render_claim(claim, definition).await?),
            claim_fence: claim.claim_fence().as_str().to_owned(),
            budget,
        }))
    }

    /// A claim response describes the accepted lease even if another writer
    /// has replaced it before this response is rendered. Its audit metadata
    /// belongs to that same accepted version, never to the current head.
    async fn render_claim(
        &self,
        claim: &StartCeremonyStepOutput,
        definition: &CeremonyDefinition,
    ) -> Result<pb::CeremonyInstanceState, tonic::Status> {
        let page = self
            .read_ceremony_events
            .execute(ReadCeremonyEventsInput::new(
                claim.instance().id().clone(),
                StreamVersion::new(claim.version().value().saturating_sub(1)),
                CeremonyEventPageLimit::new(1).map_err(domain_error_to_status)?,
            ))
            .await
            .map_err(domain_error_to_status)?;
        let head = page
            .records()
            .first()
            .filter(|record| record.sequence().value() == claim.version().value())
            .ok_or_else(|| {
                domain_error_to_status(DomainError::NotFound {
                    what: "accepted_claim_record",
                })
            })?;
        let read = CeremonyInstanceRead::new(
            claim.instance().clone(),
            head.trace_id()
                .map(TraceId::new)
                .transpose()
                .map_err(domain_error_to_status)?,
            head.correlation_id().cloned(),
            head.causation_id().cloned(),
        );
        self.render_read(&read, definition)
            .map_err(domain_error_to_status)
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

fn budget_claim_admission(output: &BudgetedStepClaimOutput) -> pb::BudgetClaimAdmission {
    pb::BudgetClaimAdmission {
        account_id: output.account_id().as_str().to_owned(),
        operation_id: output.operation_id().as_str().to_owned(),
        reservation_id: output.reservation_id().as_str().to_owned(),
    }
}
