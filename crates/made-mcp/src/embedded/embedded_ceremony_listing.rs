use made_app::usecases::CeremonyInstancePage;
use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::protocol::{tool_success_result, ToolError};
use crate::renderers::{
    CeremonyInstanceListing, CeremonyInstanceListingEntry, CeremonyInstanceSearchPage,
};

use super::EmbeddedCeremonyInstancePresenter;

pub(super) async fn present_all(made: &EmbeddedMade) -> Result<Value, ToolError> {
    let instances = made.instances().await?;
    let mut entries = Vec::with_capacity(instances.len());
    for instance in instances {
        entries.push(present_entry(made, instance.id()).await);
    }
    Ok(tool_success_result(
        CeremonyInstanceListing::new(entries).to_json(),
    ))
}

pub(super) async fn present_page(
    made: &EmbeddedMade,
    page: &CeremonyInstancePage,
) -> Result<Value, ToolError> {
    let mut entries = Vec::with_capacity(page.reads().len());
    for read in page.reads() {
        entries.push(present_entry(made, read.instance().id()).await);
    }
    Ok(tool_success_result(
        CeremonyInstanceSearchPage::new(
            entries,
            page.next_cursor().map(|cursor| cursor.as_str().to_owned()),
        )
        .to_json(),
    ))
}

async fn present_entry(
    made: &EmbeddedMade,
    id: &made_core::value_objects::CeremonyId,
) -> CeremonyInstanceListingEntry {
    match EmbeddedCeremonyInstancePresenter::present(made, id).await {
        Ok(value) => CeremonyInstanceListingEntry::rehydratable(value),
        Err(reason) => CeremonyInstanceListingEntry::unrehydratable(id.as_str(), reason.message()),
    }
}
