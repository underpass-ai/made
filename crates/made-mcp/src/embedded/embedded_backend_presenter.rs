use made_app::usecases::CeremonyInstancePage;
use made_core::value_objects::CeremonyId;
use serde_json::Value;

use super::{
    embedded_ceremony_instance_presenter::EmbeddedCeremonyInstancePresenter, EmbeddedMadeMcpBackend,
};
use crate::protocol::{tool_success_result, ToolError};
use crate::renderers::{
    CeremonyInstanceListing, CeremonyInstanceListingEntry, CeremonyInstanceSearchPage,
};

impl EmbeddedMadeMcpBackend {
    pub(super) async fn present_instance(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<Value, ToolError> {
        EmbeddedCeremonyInstancePresenter::present(&self.made, ceremony_id)
            .await
            .map(tool_success_result)
    }

    pub(super) async fn present_instances(&self) -> Result<Value, ToolError> {
        let instances = self.made.instances().await?;
        let mut values = Vec::with_capacity(instances.len());
        for instance in instances {
            match EmbeddedCeremonyInstancePresenter::present(&self.made, instance.id()).await {
                Ok(value) => values.push(CeremonyInstanceListingEntry::rehydratable(value)),
                Err(reason) => values.push(CeremonyInstanceListingEntry::unrehydratable(
                    instance.id().as_str(),
                    reason.message(),
                )),
            }
        }
        Ok(tool_success_result(
            CeremonyInstanceListing::new(values).to_json(),
        ))
    }

    pub(super) async fn present_search_page(
        &self,
        page: &CeremonyInstancePage,
    ) -> Result<Value, ToolError> {
        let mut values = Vec::with_capacity(page.reads().len());
        for read in page.reads() {
            let instance = read.instance();
            match EmbeddedCeremonyInstancePresenter::present(&self.made, instance.id()).await {
                Ok(value) => values.push(CeremonyInstanceListingEntry::rehydratable(value)),
                Err(reason) => values.push(CeremonyInstanceListingEntry::unrehydratable(
                    instance.id().as_str(),
                    reason.message(),
                )),
            }
        }
        Ok(tool_success_result(
            CeremonyInstanceSearchPage::new(
                values,
                page.next_cursor().map(|cursor| cursor.as_str().to_owned()),
            )
            .to_json(),
        ))
    }
}
