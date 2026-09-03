/// Something went wrong on the way to the kernel, or on the way back.
#[derive(Debug, thiserror::Error)]
pub enum KernelTransportError {
    #[error("the memory kernel could not be started: {0}")]
    Unstartable(String),

    #[error("the memory kernel refused the opening handshake: {0}")]
    Unwelcoming(String),

    #[error("the memory kernel stopped listening")]
    Gone,

    #[error("the memory kernel did not answer within {seconds}s")]
    Silent { seconds: u64 },

    #[error("the memory kernel answered something this client cannot read: {0}")]
    Unreadable(String),
}
