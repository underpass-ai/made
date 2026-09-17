use async_trait::async_trait;

use crate::ports::{CeremonyEventSubscriberPort, PositionedRecord};

/// The subscriber a stream gets when nothing is projecting from it.
///
/// Its own type rather than an `Option`, for the reason
/// [`NoopMetricsRecorder`](super::NoopMetricsRecorder) is one: the
/// append path notifies unconditionally, so there is no branch to get
/// wrong and no configuration in which the notification is skipped.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopCeremonyEventSubscriber;

#[async_trait]
impl CeremonyEventSubscriberPort for NoopCeremonyEventSubscriber {
    async fn observe(&self, _records: &[PositionedRecord]) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn observes_nothing_and_says_nothing() {
        NoopCeremonyEventSubscriber.observe(&[]).await;
    }
}
