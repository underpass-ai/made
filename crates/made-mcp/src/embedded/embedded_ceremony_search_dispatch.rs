use made_core::value_objects::AuthorizationRequestId;
use serde_json::Value;

use crate::protocol::ToolError;

use super::{
    embedded_ceremony_search_request::EmbeddedCeremonySearchRequest, EmbeddedMadeMcpBackend,
};

impl EmbeddedMadeMcpBackend {
    pub(super) async fn search_and_present(
        &self,
        arguments: &Value,
        request_id: AuthorizationRequestId,
    ) -> Result<Value, ToolError> {
        let page = EmbeddedCeremonySearchRequest::try_from(arguments)
            .map_err(ToolError::invalid_request)?
            .execute(&self.made, request_id)
            .await?;
        self.present_search_page(&page).await
    }
}
