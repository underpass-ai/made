use std::sync::Arc;

use made_app::usecases::PublishCouncilEventsUseCase;
use made_core::ports::{ClockPort, CouncilJournalPort, MessagingPort};

pub(super) fn journal_messaging(journal: Arc<dyn CouncilJournalPort>) -> Arc<dyn MessagingPort> {
    Arc::new(made_adapters::council_journal_messaging::CouncilJournalMessaging::new(journal))
}

pub(super) fn wire(
    enabled: bool,
    journal: Arc<dyn CouncilJournalPort>,
    messaging: Arc<dyn MessagingPort>,
    clock: Arc<dyn ClockPort>,
) -> Option<Arc<PublishCouncilEventsUseCase>> {
    enabled.then(|| Arc::new(PublishCouncilEventsUseCase::new(journal, messaging, clock)))
}
