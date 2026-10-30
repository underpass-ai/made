use made_app::usecases::ProtocolInstancePage;
use made_core::value_objects::ProtocolId;
use serde_json::Value;

use super::{
    embedded_protocol_instance_presenter::EmbeddedProtocolInstancePresenter, EmbeddedMadeMcpBackend,
};
use crate::mcp_protocol::{tool_success_result, ToolError};
use crate::renderers::{
    ProtocolInstanceListing, ProtocolInstanceListingEntry, ProtocolInstanceSearchPage,
};

impl EmbeddedMadeMcpBackend {
    pub(super) async fn present_instance(
        &self,
        protocol_id: &ProtocolId,
    ) -> Result<Value, ToolError> {
        EmbeddedProtocolInstancePresenter::present(&self.made, protocol_id)
            .await
            .map(tool_success_result)
    }

    pub(super) async fn present_instances(&self) -> Result<Value, ToolError> {
        let instances = self.made.instances().await?;
        let mut values = Vec::with_capacity(instances.len());
        for instance in instances {
            match EmbeddedProtocolInstancePresenter::present(&self.made, instance.id()).await {
                Ok(value) => values.push(ProtocolInstanceListingEntry::rehydratable(value)),
                Err(reason) => values.push(ProtocolInstanceListingEntry::unrehydratable(
                    instance.id().as_str(),
                    reason.message(),
                )),
            }
        }
        Ok(tool_success_result(
            ProtocolInstanceListing::new(values).to_json(),
        ))
    }

    pub(super) async fn present_search_page(
        &self,
        page: &ProtocolInstancePage,
    ) -> Result<Value, ToolError> {
        let mut values = Vec::with_capacity(page.reads().len());
        for read in page.reads() {
            let instance = read.instance();
            match EmbeddedProtocolInstancePresenter::present(&self.made, instance.id()).await {
                Ok(value) => values.push(ProtocolInstanceListingEntry::rehydratable(value)),
                Err(reason) => values.push(ProtocolInstanceListingEntry::unrehydratable(
                    instance.id().as_str(),
                    reason.message(),
                )),
            }
        }
        Ok(tool_success_result(
            ProtocolInstanceSearchPage::new(
                values,
                page.next_cursor().map(|cursor| cursor.as_str().to_owned()),
            )
            .to_json(),
        ))
    }
}
