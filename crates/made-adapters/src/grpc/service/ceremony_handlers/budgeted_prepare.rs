use made_app::budgets::{BudgetedStepClaimInput, BudgetedStepClaimOutput};
use made_app::usecases::{RunCeremonyStepInput, RunCeremonyStepOutput};
use made_core::entities::{CeremonyDefinition, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{ChildGroupId, ChildSpawnCoordinates, StepId};
use uuid::Uuid;

use super::super::{
    budget_error_to_status, budget_reservation_estimate_from_proto,
    claim_ceremony_step_input_from_proto, domain_error_to_status, link_span_to_metadata, pb,
    run_ceremony_step_input_from_proto, CeremonyId, GrpcResult, MadeGrpcService, Request, Response,
};

fn normalize_claim(request: &mut pb::PrepareCeremonyChildrenRequest) {
    if request.lease_owner_id.trim().is_empty() {
        "grpc-prepare-ceremony-children".clone_into(&mut request.lease_owner_id);
    }
    if request.idempotency_key.trim().is_empty() {
        request.idempotency_key = format!("grpc-prepare-{}", Uuid::new_v4());
    }
    if request.lease_ttl_ms == 0 {
        request.lease_ttl_ms = RunCeremonyStepInput::DEFAULT_LEASE_TTL_MS;
    }
}

async fn admit_budget(
    service: &MadeGrpcService,
    request: &mut pb::PrepareCeremonyChildrenRequest,
    definition: &CeremonyDefinition,
    instance: &CeremonyInstance,
) -> Result<Option<BudgetedStepClaimOutput>, tonic::Status> {
    let reservation = request.budget_reservation.take();
    if instance.budget_account_id().is_none() {
        if reservation.is_some() {
            return Err(tonic::Status::invalid_argument(
                "budget reservation supplied for an unbudgeted ceremony",
            ));
        }
        return Ok(None);
    }
    let reservation = reservation.ok_or_else(|| {
        tonic::Status::failed_precondition(
            "budgeted child preparation requires a reservation estimate",
        )
    })?;
    let estimate =
        budget_reservation_estimate_from_proto(reservation).map_err(domain_error_to_status)?;
    let claim = claim_ceremony_step_input_from_proto(
        pb::ClaimCeremonyStepRequest {
            ceremony_id: request.ceremony_id.clone(),
            step_id: request.step_id.clone(),
            actor_kind: request.actor_kind.clone(),
            lease_owner_id: request.lease_owner_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            lease_ttl_ms: request.lease_ttl_ms,
            budget_reservation: None,
        },
        definition,
        instance,
    )
    .map_err(domain_error_to_status)?;
    let output = service
        .budgeted_step_claim
        .as_deref()
        .ok_or_else(|| {
            tonic::Status::failed_precondition("budgeted step admission is not configured")
        })?
        .execute(BudgetedStepClaimInput::new(claim, estimate))
        .await
        .map_err(budget_error_to_status)?;
    Ok(Some(output))
}

async fn execute_prepare(
    service: &MadeGrpcService,
    request: &mut pb::PrepareCeremonyChildrenRequest,
    definition: &CeremonyDefinition,
    instance: &CeremonyInstance,
    step_id: &StepId,
) -> Result<RunCeremonyStepOutput, tonic::Status> {
    let input = run_ceremony_step_input_from_proto(
        pb::RunCeremonyStepRequest {
            ceremony_id: request.ceremony_id.clone(),
            step_id: request.step_id.clone(),
            actor_kind: request.actor_kind.clone(),
            lease_owner_id: request.lease_owner_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            lease_ttl_ms: request.lease_ttl_ms,
        },
        definition,
        instance,
    )
    .map_err(domain_error_to_status)?;
    if let Some(claim) = admit_budget(service, request, definition, instance).await? {
        return service
            .run_ceremony_step
            .execute_claimed_spawn(claim.claim(), step_id.clone(), input.role_kind())
            .await
            .map_err(domain_error_to_status);
    }
    service
        .run_ceremony_step
        .execute(input)
        .await
        .map_err(domain_error_to_status)
}

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.prepare_ceremony_children", skip_all)]
    pub(crate) async fn handle_prepare_ceremony_children(
        &self,
        request: Request<pb::PrepareCeremonyChildrenRequest>,
    ) -> GrpcResult<pb::PrepareCeremonyChildrenResponse> {
        link_span_to_metadata(&request);
        let mut request = request.into_inner();
        normalize_claim(&mut request);
        let ceremony_id =
            CeremonyId::new(request.ceremony_id.clone()).map_err(domain_error_to_status)?;
        let step_id = StepId::new(request.step_id.clone()).map_err(domain_error_to_status)?;
        let (instance, definition) = self.session(&ceremony_id).await?;
        let output = Box::pin(execute_prepare(
            self,
            &mut request,
            &definition,
            &instance,
            &step_id,
        ))
        .await?;
        let record = output.instance().step_record(&step_id).ok_or_else(|| {
            domain_error_to_status(DomainError::NotFound {
                what: "ceremony_step_record",
            })
        })?;
        let coordinates = ChildSpawnCoordinates::new(
            step_id,
            output.instance().current_state_visit(),
            output.instance().current_state_iteration(),
            record.iteration(),
        );
        let group_id = ChildGroupId::derive(output.instance().id(), &coordinates);
        let group = output.instance().child_group(&group_id).ok_or_else(|| {
            domain_error_to_status(DomainError::NotFound {
                what: "child_spawn_group",
            })
        })?;
        Ok(Response::new(pb::PrepareCeremonyChildrenResponse {
            instance: Some(self.render(output.instance(), &definition).await?),
            child_group_id: group_id.as_str().to_owned(),
            child_ids: group
                .plan()
                .children()
                .iter()
                .map(|child| child.child_id().as_str().to_owned())
                .collect(),
        }))
    }
}
