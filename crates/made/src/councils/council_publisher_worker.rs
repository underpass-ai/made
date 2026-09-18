use made_app::usecases::PublishCouncilEventsUseCase;
use made_core::value_objects::{CouncilJournalConsumer, CouncilJournalPageLimit};
use std::sync::Arc;
use tokio::sync::watch;

/// A bounded polling host for the council outbox. Periodic recovery does not
/// depend on another mutation or a broker notification waking the process.
pub(crate) struct CouncilPublisherWorker {
    publisher: Arc<PublishCouncilEventsUseCase>,
    consumer: CouncilJournalConsumer,
}
impl CouncilPublisherWorker {
    pub(crate) fn new(
        publisher: Arc<PublishCouncilEventsUseCase>,
        consumer: CouncilJournalConsumer,
    ) -> Self {
        Self {
            publisher,
            consumer,
        }
    }
    pub(crate) async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            if *shutdown.borrow() {
                break;
            }
            tokio::select! {
                changed = shutdown.changed() => { if changed.is_err() || *shutdown.borrow() { break; } }
                _ = interval.tick() => {
                    let result = tokio::select! {
                        changed = shutdown.changed() => { if changed.is_err() || *shutdown.borrow() { break; } continue; }
                        result = self.publisher.execute(&self.consumer, CouncilJournalPageLimit::default()) => result,
                    };
                    if let Err(error) = result {
                        tracing::warn!(consumer = self.consumer.as_str(), %error, "council outbox remains pending; retrying on next poll");
                    }
                }
            }
        }
    }
}
