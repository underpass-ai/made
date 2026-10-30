use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use made_core::error::DomainError;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;

use super::ProtocolProgressFrame;

/// A progress receiver whose drop cancels its producer immediately.
pub struct ProtocolProgressStream {
    receiver: ReceiverStream<Result<ProtocolProgressFrame, DomainError>>,
    producer: JoinHandle<()>,
}

impl ProtocolProgressStream {
    pub(crate) fn new(
        receiver: mpsc::Receiver<Result<ProtocolProgressFrame, DomainError>>,
        producer: JoinHandle<()>,
    ) -> Self {
        Self {
            receiver: ReceiverStream::new(receiver),
            producer,
        }
    }
}

impl Stream for ProtocolProgressStream {
    type Item = Result<ProtocolProgressFrame, DomainError>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(context)
    }
}

impl Drop for ProtocolProgressStream {
    fn drop(&mut self) {
        self.producer.abort();
    }
}

impl std::fmt::Debug for ProtocolProgressStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ProtocolProgressStream").finish()
    }
}
