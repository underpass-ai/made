use made_core::error::DomainError;
use made_core::value_objects::{MaxParallel, StepFailureKind};
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry};

/// Registry families owned by the sealed-event concurrency projection.
pub(super) struct CeremonyConcurrencyMetrics {
    width: HistogramVec,
    failures: IntCounterVec,
}

impl CeremonyConcurrencyMetrics {
    pub(super) fn new(registry: &Registry) -> Result<Self, DomainError> {
        let width = HistogramVec::new(
            HistogramOpts::new("made_ceremony_claim_peak_width", "Peak live step leases per finished state visit/iteration observed from the stream.")
                .buckets(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]),
            &["ceremony", "state"],
        ).map_err(|failure| error(&failure))?;
        let failures = IntCounterVec::new(
            Opts::new(
                "made_ceremony_step_failure_total",
                "Classified step failures sealed in the ceremony stream.",
            ),
            &["ceremony", "step", "failure_kind"],
        )
        .map_err(|failure| error(&failure))?;
        registry
            .register(Box::new(width.clone()))
            .map_err(|failure| error(&failure))?;
        registry
            .register(Box::new(failures.clone()))
            .map_err(|failure| error(&failure))?;
        Ok(Self { width, failures })
    }

    pub(super) fn observe_width(&self, ceremony: &str, state: &str, width: MaxParallel) {
        self.width
            .with_label_values(&[ceremony, state])
            .observe(f64::from(width.get()));
    }

    pub(super) fn record_failure(&self, ceremony: &str, step: &str, kind: StepFailureKind) {
        self.failures
            .with_label_values(&[ceremony, step, kind.as_label()])
            .inc();
    }
}

fn error(error: &prometheus::Error) -> DomainError {
    DomainError::InvalidDocument {
        reason: format!("concurrency metric registration: {error}"),
    }
}
