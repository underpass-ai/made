use made_app::budgets::StartBudgetedProtocolInput;
use made_core::entities::ProtocolInstance;

use super::super::{
    budget_error_to_status, budget_limits_from_proto, domain_error_to_status, pb,
    start_published_protocol_input_from_proto, MadeGrpcService, Status,
};

pub(super) async fn execute(
    service: &MadeGrpcService,
    request: pb::StartPublishedProtocolRequest,
) -> Result<ProtocolInstance, Status> {
    let limits = request
        .budget_limits
        .clone()
        .map(budget_limits_from_proto)
        .transpose()
        .map_err(domain_error_to_status)?;
    let input =
        start_published_protocol_input_from_proto(request).map_err(domain_error_to_status)?;
    if let Some(limits) = limits {
        return service
            .start_budgeted_protocol
            .as_deref()
            .ok_or_else(|| {
                Status::failed_precondition("budgeted protocol admission is not configured")
            })?
            .execute(StartBudgetedProtocolInput::new(input, limits))
            .await
            .map_err(budget_error_to_status);
    }
    service
        .start_published_protocol
        .execute(input)
        .await
        .map_err(domain_error_to_status)
}
