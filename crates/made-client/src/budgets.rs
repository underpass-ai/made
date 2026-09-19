use made_proto::v1::{
    GetBudgetReportRequest, GetBudgetReportResponse, ListPendingBudgetReservationsRequest,
    ListPendingBudgetReservationsResponse,
};

use crate::{MadeClient, MadeClientError};

impl MadeClient {
    /// Read the durable budget account shared by a ceremony tree.
    pub async fn get_budget_report(
        &self,
        ceremony_id: impl Into<String>,
    ) -> Result<GetBudgetReportResponse, MadeClientError> {
        self.rpc()
            .get_budget_report(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/GetBudgetReport",
                GetBudgetReportRequest {
                    ceremony_id: ceremony_id.into(),
                },
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }

    /// List unresolved reservations in stable keyset order.
    pub async fn list_pending_budget_reservations(
        &self,
        after_reservation_id: impl Into<String>,
        limit: u32,
    ) -> Result<ListPendingBudgetReservationsResponse, MadeClientError> {
        self.rpc()
            .list_pending_budget_reservations(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/ListPendingBudgetReservations",
                ListPendingBudgetReservationsRequest {
                    after_reservation_id: after_reservation_id.into(),
                    limit,
                },
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }
}
