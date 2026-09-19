use made_app::budgets::StartBudgetedCeremonyInput;
use made_core::entities::CeremonyInstance;

use super::super::{
    budget_error_to_status, budget_limits_from_proto, domain_error_to_status, pb,
    start_published_ceremony_input_from_proto, MadeGrpcService, Status,
};

pub(super) async fn execute(
    service: &MadeGrpcService,
    request: pb::StartPublishedCeremonyRequest,
) -> Result<CeremonyInstance, Status> {
    let limits = request
        .budget_limits
        .clone()
        .map(budget_limits_from_proto)
        .transpose()
        .map_err(domain_error_to_status)?;
    let input =
        start_published_ceremony_input_from_proto(request).map_err(domain_error_to_status)?;
    if let Some(limits) = limits {
        return service
            .start_budgeted_ceremony
            .as_deref()
            .ok_or_else(|| {
                Status::failed_precondition("budgeted ceremony admission is not configured")
            })?
            .execute(StartBudgetedCeremonyInput::new(input, limits))
            .await
            .map_err(budget_error_to_status);
    }
    service
        .start_published_ceremony
        .execute(input)
        .await
        .map_err(domain_error_to_status)
}
