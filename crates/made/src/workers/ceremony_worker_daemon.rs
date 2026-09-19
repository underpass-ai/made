use std::sync::Arc;

use made_app::workers::{
    CeremonyWorkerHost, CeremonyWorkerHostPolicy, CeremonyWorkerRunOutcome,
    CeremonyWorkerStopToken, ClaimCeremonyWorkInput,
};
use made_core::DomainError;

/// Installed worker daemon. The runtime owns its task and requests cooperative drain on shutdown.
pub struct CeremonyWorkerDaemon {
    host: Arc<CeremonyWorkerHost>,
    input: ClaimCeremonyWorkInput,
    policy: CeremonyWorkerHostPolicy,
    stop: CeremonyWorkerStopToken,
}

impl CeremonyWorkerDaemon {
    #[must_use]
    pub const fn new(
        host: Arc<CeremonyWorkerHost>,
        input: ClaimCeremonyWorkInput,
        policy: CeremonyWorkerHostPolicy,
        stop: CeremonyWorkerStopToken,
    ) -> Self {
        Self {
            host,
            input,
            policy,
            stop,
        }
    }

    pub async fn run(&self) -> Result<CeremonyWorkerRunOutcome, DomainError> {
        self.host
            .run_continuously(self.input.clone(), self.policy)
            .await
    }

    pub fn request_stop(&self) {
        self.stop.request();
    }
}

impl std::fmt::Debug for CeremonyWorkerDaemon {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonyWorkerDaemon")
            .field("stop_requested", &self.stop.is_requested())
            .finish_non_exhaustive()
    }
}
