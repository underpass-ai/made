use std::future::{poll_fn, Future};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};

/// Cooperative loss-of-authority signal. Process adapters must kill and reap
/// their owned execution before returning from cancellation.
#[derive(Debug, Clone)]
pub struct ExecutionCancellation {
    state: Arc<Mutex<Option<Vec<Waker>>>>,
}

impl ExecutionCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(Some(Vec::new()))),
        }
    }

    pub fn cancel(&self) {
        let waiters = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        for waker in waiters.into_iter().flatten() {
            waker.wake();
        }
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none()
    }

    pub async fn cancelled(&self) {
        poll_fn(|cx| {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match state.as_mut() {
                None => Poll::Ready(()),
                Some(waiters) => {
                    if !waiters.iter().any(|w| w.will_wake(cx.waker())) {
                        waiters.push(cx.waker().clone());
                    }
                    Poll::Pending
                }
            }
        })
        .await;
    }

    /// Cancels a future without asserting that dropping it kills external work.
    pub async fn run<F: Future>(&self, future: F) -> Option<F::Output> {
        let mut future = std::pin::pin!(future);
        let mut cancelled = std::pin::pin!(self.cancelled());
        poll_fn(|cx| {
            if cancelled.as_mut().poll(cx).is_ready() {
                return Poll::Ready(None);
            }
            future.as_mut().poll(cx).map(Some)
        })
        .await
    }
}

impl Default for ExecutionCancellation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ExecutionCancellation;

    #[tokio::test]
    async fn cancellation_wakes_every_waiter() {
        let cancellation = ExecutionCancellation::new();
        let left = cancellation.clone();
        let right = cancellation.clone();
        let left = tokio::spawn(async move { left.cancelled().await });
        let right = tokio::spawn(async move { right.cancelled().await });

        tokio::task::yield_now().await;
        cancellation.cancel();
        left.await.unwrap();
        right.await.unwrap();
        assert!(cancellation.is_cancelled());
    }

    #[tokio::test]
    async fn run_returns_none_after_authority_is_cancelled() {
        let cancellation = ExecutionCancellation::new();
        let trigger = cancellation.clone();
        let result = cancellation
            .run(async move {
                trigger.cancel();
                tokio::task::yield_now().await;
                42
            })
            .await;
        assert_eq!(result, None);
    }
}
