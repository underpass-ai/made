use std::time::Duration;

/// Bounded connection policy for the public MADE endpoint.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    endpoint: String,
    connect_attempts: u32,
    initial_backoff: Duration,
    maximum_backoff: Duration,
}

impl ClientConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            connect_attempts: 5,
            initial_backoff: Duration::from_millis(100),
            maximum_backoff: Duration::from_secs(2),
        }
    }

    #[must_use]
    pub fn with_connect_attempts(mut self, attempts: u32) -> Self {
        self.connect_attempts = attempts.max(1);
        self
    }

    #[must_use]
    pub fn with_backoff(mut self, initial: Duration, maximum: Duration) -> Self {
        self.initial_backoff = initial;
        self.maximum_backoff = maximum.max(initial);
        self
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn connect_attempts(&self) -> u32 {
        self.connect_attempts
    }

    pub(crate) fn initial_backoff(&self) -> Duration {
        self.initial_backoff
    }

    pub(crate) fn maximum_backoff(&self) -> Duration {
        self.maximum_backoff
    }
}
