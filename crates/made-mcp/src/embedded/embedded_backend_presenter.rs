use made_core::value_objects::CeremonyId;
use serde_json::Value;

use super::{
    embedded_ceremony_instance_presenter::EmbeddedCeremonyInstancePresenter, EmbeddedMadeMcpBackend,
};
use crate::protocol::{tool_success_result, ToolError};
use crate::renderers::{CeremonyInstanceListing, CeremonyInstanceListingEntry};

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
}
