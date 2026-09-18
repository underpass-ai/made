use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use made_core::error::DomainError;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;

use super::CeremonyProgressFrame;

/// A progress receiver whose drop cancels its producer immediately.
pub struct CeremonyProgressStream {
    receiver: ReceiverStream<Result<CeremonyProgressFrame, DomainError>>,
    producer: JoinHandle<()>,
}

impl CeremonyProgressStream {
    pub(crate) fn new(
        receiver: mpsc::Receiver<Result<CeremonyProgressFrame, DomainError>>,
        producer: JoinHandle<()>,
    ) -> Self {
        Self {
            receiver: ReceiverStream::new(receiver),
            producer,
        }
    }
}

impl Stream for CeremonyProgressStream {
    type Item = Result<CeremonyProgressFrame, DomainError>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(context)
    }
}

impl Drop for CeremonyProgressStream {
    fn drop(&mut self) {
        self.producer.abort();
    }
}

impl std::fmt::Debug for CeremonyProgressStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CeremonyProgressStream").finish()
    }
}
