use made_core::value_objects::{BudgetPageLimit, BudgetReservationId, CeremonyId};

use super::{
    budget_balance_to_proto, budget_error_to_status, budget_reservation_to_proto,
    domain_error_to_status, link_span_to_metadata, pb, GrpcResult, MadeGrpcService, Request,
    Response, Status,
};

const DEFAULT_PAGE_LIMIT: usize = 100;

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.get_budget_report", skip_all)]
    pub(super) async fn handle_get_budget_report(
        &self,
        request: Request<pb::GetBudgetReportRequest>,
    ) -> GrpcResult<pb::GetBudgetReportResponse> {
        link_span_to_metadata(&request);
        let ceremony_id =
            CeremonyId::new(request.into_inner().ceremony_id).map_err(domain_error_to_status)?;
        let instance = self
            .get_ceremony_instance
            .execute(&ceremony_id)
            .await
            .map_err(domain_error_to_status)?;
        let account = instance
            .budget_account_id()
            .ok_or_else(|| Status::failed_precondition("ceremony has no durable budget account"))?;
        let balance = self
            .budget_service()
            .ok_or_else(|| Status::failed_precondition("budget ledger is not configured"))?
            .report(account)
            .await
            .map_err(budget_error_to_status)?;
        Ok(Response::new(pb::GetBudgetReportResponse {
            account_id: account.as_str().to_owned(),
            balance: Some(budget_balance_to_proto(&balance)),
        }))
    }

    #[tracing::instrument(name = "rpc.list_pending_budget_reservations", skip_all)]
    pub(super) async fn handle_list_pending_budget_reservations(
        &self,
        request: Request<pb::ListPendingBudgetReservationsRequest>,
    ) -> GrpcResult<pb::ListPendingBudgetReservationsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let after = (!request.after_reservation_id.trim().is_empty())
            .then(|| BudgetReservationId::new(request.after_reservation_id))
            .transpose()
            .map_err(domain_error_to_status)?;
        let limit = if request.limit == 0 {
            BudgetPageLimit::new(DEFAULT_PAGE_LIMIT)
        } else {
            BudgetPageLimit::new(request.limit as usize)
        }
        .map_err(domain_error_to_status)?;
        let page = self
            .budget_service()
            .ok_or_else(|| Status::failed_precondition("budget ledger is not configured"))?
            .pending(after.as_ref(), limit)
            .await
            .map_err(budget_error_to_status)?;
        let next_cursor = page
            .reservations()
            .last()
            .map_or_else(String::new, |reservation| {
                reservation.id().as_str().to_owned()
            });
        Ok(Response::new(pb::ListPendingBudgetReservationsResponse {
            reservations: page
                .reservations()
                .iter()
                .map(budget_reservation_to_proto)
                .collect(),
            next_cursor,
        }))
    }

    fn budget_service(&self) -> Option<&made_app::budgets::BudgetLedgerService> {
        self.budgets.as_deref()
    }
}
