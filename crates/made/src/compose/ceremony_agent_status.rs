use std::sync::Arc;

use made_adapters::memory::InMemoryCeremonyAgentStatus;
use made_app::services::SessionStream;
use made_app::usecases::CeremonyAgentStatusService;
use made_core::ports::{CeremonyAgentStatusPort, ClockPort};
use time::Duration;

const DEFAULT_STALE_AFTER: Duration = Duration::seconds(60);

/// The roster itself, which nothing else has to exist first.
///
/// Split from the service because the projection that fills the
/// delivery ledger is a subscriber of the stream, and the stream cannot
/// be built after something that needs the stream. The port has no such
/// dependency; the service, which verifies claims against the journal,
/// does.
pub(super) fn port() -> Arc<dyn CeremonyAgentStatusPort> {
    Arc::new(InMemoryCeremonyAgentStatus::new())
}

pub(super) fn service<C: ClockPort + 'static>(
    port: Arc<dyn CeremonyAgentStatusPort>,
    clock: &Arc<C>,
    journal: &Arc<SessionStream>,
) -> Arc<CeremonyAgentStatusService> {
    Arc::new(
        CeremonyAgentStatusService::new(port, clock.clone(), DEFAULT_STALE_AFTER)
            .with_journal_claims(journal.clone()),
    )
}
