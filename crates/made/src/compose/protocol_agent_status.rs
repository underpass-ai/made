use std::sync::Arc;

use made_adapters::memory::InMemoryProtocolAgentStatus;
use made_app::services::SessionStream;
use made_app::usecases::ProtocolAgentStatusService;
use made_core::ports::{ClockPort, ProtocolAgentStatusPort};
use time::Duration;

const DEFAULT_STALE_AFTER: Duration = Duration::seconds(60);

/// The roster itself, which nothing else has to exist first.
///
/// Split from the service because the projection that fills the
/// delivery ledger is a subscriber of the stream, and the stream cannot
/// be built after something that needs the stream. The port has no such
/// dependency; the service, which verifies claims against the journal,
/// does.
pub(super) fn port() -> Arc<dyn ProtocolAgentStatusPort> {
    Arc::new(InMemoryProtocolAgentStatus::new())
}

pub(super) fn service<C: ClockPort + 'static>(
    port: Arc<dyn ProtocolAgentStatusPort>,
    clock: &Arc<C>,
    journal: &Arc<SessionStream>,
) -> Arc<ProtocolAgentStatusService> {
    Arc::new(
        ProtocolAgentStatusService::new(port, clock.clone(), DEFAULT_STALE_AFTER)
            .with_journal_claims(journal.clone()),
    )
}
