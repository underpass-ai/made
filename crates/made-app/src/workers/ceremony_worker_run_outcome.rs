/// Aggregate counters from one cooperative continuous-host run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CeremonyWorkerRunOutcome {
    claim_pages: u64,
    recovery_pages: u64,
    completed: u64,
    reconciliation_required: u64,
    claim_failures: u64,
    item_failures: u64,
    retries: u64,
    stopped: bool,
}

impl CeremonyWorkerRunOutcome {
    /// Compatibility summary for the original host loop API.
    #[must_use]
    pub fn new(cycles: u64, stopped: bool) -> Self {
        Self {
            claim_pages: cycles,
            stopped,
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn claim_pages(self) -> u64 {
        self.claim_pages
    }

    #[must_use]
    pub const fn recovery_pages(self) -> u64 {
        self.recovery_pages
    }

    #[must_use]
    pub const fn completed(self) -> u64 {
        self.completed
    }

    #[must_use]
    pub const fn reconciliation_required(self) -> u64 {
        self.reconciliation_required
    }

    #[must_use]
    pub const fn claim_failures(self) -> u64 {
        self.claim_failures
    }

    #[must_use]
    pub const fn item_failures(self) -> u64 {
        self.item_failures
    }

    #[must_use]
    pub const fn retries(self) -> u64 {
        self.retries
    }

    #[must_use]
    pub const fn stopped(self) -> bool {
        self.stopped
    }

    pub(crate) fn record_claim_page(&mut self, page: &super::CeremonyWorkerHostOutcome) {
        self.claim_pages = self.claim_pages.saturating_add(1);
        self.claim_failures = self
            .claim_failures
            .saturating_add(page.claim_failures().len() as u64);
        self.record_batch(page.batch());
    }

    pub(crate) fn record_recovery_page(&mut self, batch: &super::CeremonyWorkerBatchOutcome) {
        self.recovery_pages = self.recovery_pages.saturating_add(1);
        self.record_batch(batch);
    }

    pub(crate) fn record_retry(&mut self) {
        self.retries = self.retries.saturating_add(1);
    }

    pub(crate) fn finish(&mut self, stopped: bool) {
        self.stopped = stopped;
    }

    fn record_batch(&mut self, batch: &super::CeremonyWorkerBatchOutcome) {
        self.item_failures = self
            .item_failures
            .saturating_add(batch.failures().len() as u64);
        for outcome in batch.outcomes() {
            match outcome {
                super::RecoverableCeremonyWorkerOutcome::Completed { .. } => {
                    self.completed = self.completed.saturating_add(1);
                }
                super::RecoverableCeremonyWorkerOutcome::ReconciliationRequired(_) => {
                    self.reconciliation_required = self.reconciliation_required.saturating_add(1);
                }
            }
        }
    }
}
