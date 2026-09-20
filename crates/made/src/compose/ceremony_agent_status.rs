use std::sync::Arc;

use made_adapters::memory::InMemoryCeremonyAgentStatus;
use made_app::services::SessionStream;
use made_app::usecases::CeremonyAgentStatusService;
use made_core::ports::{CeremonyAgentStatusPort, ClockPort};
use time::Duration;

const DEFAULT_STALE_AFTER: Duration = Duration::seconds(60);

pub(super) fn wire(
    clock: Arc<dyn ClockPort>,
    journal: Arc<SessionStream>,
) -> (
    Arc<CeremonyAgentStatusService>,
    Arc<dyn CeremonyAgentStatusPort>,
) {
    let port = Arc::new(InMemoryCeremonyAgentStatus::new());
    let service = Arc::new(
        CeremonyAgentStatusService::new(port.clone(), clock, DEFAULT_STALE_AFTER)
            .with_journal_claims(journal),
    );
    (service, port)
}
