use async_trait::async_trait;

use crate::ports::{PositionedRecord, ProtocolEventSubscriberPort};

/// The subscriber a stream gets when nothing is projecting from it.
///
/// Its own type rather than an `Option`, for the reason
/// [`NoopMetricsRecorder`](super::NoopMetricsRecorder) is one: the
/// append path notifies unconditionally, so there is no branch to get
/// wrong and no configuration in which the notification is skipped.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopProtocolEventSubscriber;

#[async_trait]
impl ProtocolEventSubscriberPort for NoopProtocolEventSubscriber {
    async fn observe(&self, _records: &[PositionedRecord]) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn observes_nothing_and_says_nothing() {
        NoopProtocolEventSubscriber.observe(&[]).await;
    }
}
