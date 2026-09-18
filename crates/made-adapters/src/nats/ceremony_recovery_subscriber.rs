use std::sync::Arc;
use std::time::Duration;

use async_nats::Client;
use futures::{Stream, StreamExt};
use made_app::usecases::RecoverCeremonyChildrenUseCase;
use made_core::error::DomainError;
use made_core::value_objects::CeremonyEventPageLimit;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use super::{ceremony_recovery_cursor::RecoveryCursor, NatsSubjects};

const READINESS_DEADLINE: Duration = Duration::from_secs(5);
const READINESS_MARKER: &[u8] = b"made-children-recovery-ready";
/// Bounds recovery latency when a best-effort NATS wake is lost or arrives
/// while another process owns the durable cursor lease.
const CATCH_UP_INTERVAL: Duration = Duration::from_secs(1);

/// Wakes the durable child recovery cursor when ceremony events arrive.
///
/// The broker payload is deliberately ignored. Every effect is rebuilt from
/// the global event store and acknowledged through the recovery use case's
/// owned cursor, so dropped or duplicate notifications cannot change truth.
#[derive(Clone)]
pub struct NatsCeremonyRecoverySubscriber {
    client: Client,
    subject: String,
    recover: Arc<RecoverCeremonyChildrenUseCase>,
}

impl NatsCeremonyRecoverySubscriber {
    #[must_use]
    pub fn new(
        client: Client,
        subjects: &NatsSubjects,
        recover: Arc<RecoverCeremonyChildrenUseCase>,
    ) -> Self {
        Self {
            client,
            subject: format!("{}.>", subjects.ceremony_prefix),
            recover,
        }
    }

    /// Subscribe, prove the server processed that SUB, then start the loop.
    ///
    /// A client flush only drains local writes. Receiving a marker on a
    /// private inbox proves a server round trip on the same connection and,
    /// by command ordering, that the earlier business subscription is live.
    pub async fn spawn(self) -> Result<JoinHandle<()>, DomainError> {
        let messages = self
            .client
            .subscribe(self.subject.clone())
            .await
            .map_err(|_| DomainError::InvariantViolated {
                reason: "nats: ceremony recovery subscribe failed",
            })?;
        self.wait_until_ready().await?;
        let recover: Arc<dyn RecoveryCursor> = self.recover;
        let subject = self.subject;
        Ok(tokio::spawn(async move {
            info!(subject, "nats ceremony recovery subscriber started");
            run_wake_loop(messages.map(|_| ()), recover, CATCH_UP_INTERVAL).await;
            info!(subject, "nats ceremony recovery subscriber stream ended");
        }))
    }

    async fn wait_until_ready(&self) -> Result<(), DomainError> {
        tokio::time::timeout(READINESS_DEADLINE, async {
            let inbox = self.client.new_inbox();
            let mut barrier = self.client.subscribe(inbox.clone()).await.map_err(|_| {
                DomainError::InvariantViolated {
                    reason: "nats: ceremony recovery readiness subscribe failed",
                }
            })?;
            self.client
                .publish(inbox, READINESS_MARKER.into())
                .await
                .map_err(|_| DomainError::InvariantViolated {
                    reason: "nats: ceremony recovery readiness publish failed",
                })?;
            let marker = barrier.next().await.ok_or(DomainError::InvariantViolated {
                reason: "nats: ceremony recovery readiness inbox closed",
            })?;
            if marker.payload.as_ref() != READINESS_MARKER {
                return Err(DomainError::InvariantViolated {
                    reason: "nats: ceremony recovery readiness marker mismatch",
                });
            }
            Ok(())
        })
        .await
        .map_err(|_| DomainError::InvariantViolated {
            reason: "nats: ceremony recovery readiness timed out",
        })?
    }
}

impl std::fmt::Debug for NatsCeremonyRecoverySubscriber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NatsCeremonyRecoverySubscriber")
            .field("subject", &self.subject)
            .finish_non_exhaustive()
    }
}

async fn run_wake_loop<S>(
    mut messages: S,
    recover: Arc<dyn RecoveryCursor>,
    catch_up_interval: Duration,
) where
    S: Stream<Item = ()> + Unpin,
{
    drain(recover.as_ref()).await;
    let mut catch_up = tokio::time::interval(catch_up_interval);
    catch_up.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    catch_up.tick().await;
    loop {
        tokio::select! {
            message = messages.next() => {
                if message.is_none() {
                    break;
                }
                drain(recover.as_ref()).await;
            }
            _ = catch_up.tick() => drain(recover.as_ref()).await,
        }
    }
}

async fn drain(recover: &dyn RecoveryCursor) {
    let limit = CeremonyEventPageLimit::DEFAULT;
    loop {
        match recover.recover(limit).await {
            Ok(round) if round.failed > 0 => {
                warn!(
                    failed = round.failed,
                    "child recovery cursor remains pending"
                );
                break;
            }
            Ok(round) if round.busy || round.acknowledged() < limit.value() => break,
            Ok(_) => {}
            Err(error) => {
                warn!(error = %error, "child recovery wake failed");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use async_trait::async_trait;
    use futures::stream;
    use made_app::usecases::RecoverCeremonyChildrenRound;

    use super::*;

    #[derive(Debug, Default)]
    struct RecordingRecoveryCursor {
        calls: AtomicUsize,
        fail_first: AtomicBool,
    }

    impl RecordingRecoveryCursor {
        fn failing_once() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                fail_first: AtomicBool::new(true),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl RecoveryCursor for RecordingRecoveryCursor {
        async fn recover(
            &self,
            _limit: CeremonyEventPageLimit,
        ) -> Result<RecoverCeremonyChildrenRound, DomainError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_first.swap(false, Ordering::SeqCst) {
                return Err(DomainError::InvariantViolated {
                    reason: "transient recovery failure",
                });
            }
            Ok(RecoverCeremonyChildrenRound::default())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_durable_event_is_rechecked_periodically_without_a_broker_wake() {
        let cursor = Arc::new(RecordingRecoveryCursor::default());
        let task = tokio::spawn(run_wake_loop(
            stream::pending(),
            cursor.clone(),
            CATCH_UP_INTERVAL,
        ));
        tokio::task::yield_now().await;
        assert_eq!(cursor.calls(), 1, "startup catch-up did not run");

        tokio::time::advance(CATCH_UP_INTERVAL).await;
        tokio::task::yield_now().await;
        assert_eq!(cursor.calls(), 2, "the periodic catch-up did not run");

        task.abort();
        let _ = task.await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_transient_drain_failure_is_retried_without_another_broker_event() {
        let cursor = Arc::new(RecordingRecoveryCursor::failing_once());
        let task = tokio::spawn(run_wake_loop(
            stream::pending(),
            cursor.clone(),
            CATCH_UP_INTERVAL,
        ));
        tokio::task::yield_now().await;
        assert_eq!(cursor.calls(), 1, "the injected failure did not run");

        tokio::time::advance(CATCH_UP_INTERVAL).await;
        tokio::task::yield_now().await;
        assert_eq!(cursor.calls(), 2, "the failed cursor was not retried");

        task.abort();
        let _ = task.await;
    }
}
