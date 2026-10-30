use std::sync::Arc;

use made_app::workers::{
    ClaimProtocolWorkInput, ProtocolWorkerHost, ProtocolWorkerHostPolicy, ProtocolWorkerRunOutcome,
    ProtocolWorkerStopToken,
};
use made_core::DomainError;

/// Installed worker daemon. The runtime owns its task and requests cooperative drain on shutdown.
pub struct ProtocolWorkerDaemon {
    host: Arc<ProtocolWorkerHost>,
    input: ClaimProtocolWorkInput,
    policy: ProtocolWorkerHostPolicy,
    stop: ProtocolWorkerStopToken,
}

impl ProtocolWorkerDaemon {
    #[must_use]
    pub const fn new(
        host: Arc<ProtocolWorkerHost>,
        input: ClaimProtocolWorkInput,
        policy: ProtocolWorkerHostPolicy,
        stop: ProtocolWorkerStopToken,
    ) -> Self {
        Self {
            host,
            input,
            policy,
            stop,
        }
    }

    pub async fn run(&self) -> Result<ProtocolWorkerRunOutcome, DomainError> {
        self.host
            .run_continuously(self.input.clone(), self.policy)
            .await
    }

    pub fn request_stop(&self) {
        self.stop.request();
    }
}

impl std::fmt::Debug for ProtocolWorkerDaemon {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProtocolWorkerDaemon")
            .field("stop_requested", &self.stop.is_requested())
            .finish_non_exhaustive()
    }
}
