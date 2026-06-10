//! NATS [`MessagingPort`] implementation.
//!
//! Publishes every outbound domain event as a JSON message on its
//! canonical subject. The payload is whatever `serde_json` produces
//! for the event type; thanks to `#[serde(flatten)]` on the envelope
//! field the shape matches the AsyncAPI contract (envelope at the
//! root next to event-specific fields).
//!
//! Every outbound message also carries a W3C Trace Context
//! `traceparent` NATS header (generated per publish when no upstream
//! context is available). Downstream OTel-aware consumers can stitch
//! the trace hierarchy from this header; the inbound subscriber in
//! this crate reads the same header back and surfaces it as span
//! fields on `nats.trigger.inbound`.

use std::sync::Arc;
use std::time::Instant;

use async_nats::{header::HeaderMap, Client};
use async_trait::async_trait;
use choreo_core::error::DomainError;
use choreo_core::events::{
    DeliberationCompletedEvent, PhaseChangedEvent, TaskCompletedEvent, TaskDispatchedEvent,
    TaskFailedEvent,
};
use choreo_core::ports::{MessagingPort, MetricsRecorderPort, NoopMetricsRecorder};
use choreo_core::value_objects::{DurationMs, TraceContext};
use serde::Serialize;
use tracing::debug;

use super::config::NatsSubjects;

/// NATS header name for W3C Trace Context propagation.
pub(super) const TRACEPARENT_HEADER: &str = "traceparent";

/// Publishes domain events to NATS.
///
/// Constructed from an already-connected [`Client`]; the composition
/// root owns connection lifecycle so the adapter stays focused.
#[derive(Clone)]
pub struct NatsMessaging {
    client: Client,
    subjects: NatsSubjects,
    metrics: Arc<dyn MetricsRecorderPort>,
}

impl std::fmt::Debug for NatsMessaging {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatsMessaging")
            .field("subjects", &self.subjects)
            .finish()
    }
}

impl NatsMessaging {
    #[must_use]
    pub fn new(client: Client, subjects: NatsSubjects) -> Self {
        Self {
            client,
            subjects,
            metrics: Arc::new(NoopMetricsRecorder),
        }
    }

    /// Attach a metrics recorder so publish latency and failures are
    /// counted. The composition root wires the real recorder; the default
    /// no-op keeps tests free of one.
    #[must_use]
    pub fn with_metrics(mut self, metrics: Arc<dyn MetricsRecorderPort>) -> Self {
        self.metrics = metrics;
        self
    }

    async fn publish_event<E: Serialize>(
        &self,
        subject_kind: &'static str,
        subject: &str,
        event: &E,
    ) -> Result<(), DomainError> {
        let payload = serde_json::to_vec(event).map_err(|err| {
            self.metrics
                .record_nats_publish_error(subject_kind, "serialize");
            debug!(error = %err, "nats payload encoding failed");
            DomainError::InvariantViolated {
                reason: "nats: failed to serialize outbound event",
            }
        })?;
        let trace = TraceContext::generate();
        let mut headers = HeaderMap::new();
        headers.insert(TRACEPARENT_HEADER, trace.to_header().as_str());
        let started = Instant::now();
        let result = self
            .client
            .publish_with_headers(subject.to_owned(), headers, payload.into())
            .await;
        self.metrics.observe_nats_publish(
            subject_kind,
            DurationMs::from_millis(u64::try_from(started.elapsed().as_millis()).unwrap_or(0)),
        );
        result.map_err(|err| {
            self.metrics
                .record_nats_publish_error(subject_kind, "publish");
            debug!(error = %err, subject, "nats publish failed");
            DomainError::InvariantViolated {
                reason: "nats: publish failed",
            }
        })?;
        debug!(subject, trace_id = trace.trace_id(), "nats event published");
        Ok(())
    }
}

#[async_trait]
impl MessagingPort for NatsMessaging {
    async fn publish_task_dispatched(
        &self,
        event: &TaskDispatchedEvent,
    ) -> Result<(), DomainError> {
        self.publish_event("task_dispatched", &self.subjects.task_dispatched, event)
            .await
    }

    async fn publish_task_completed(&self, event: &TaskCompletedEvent) -> Result<(), DomainError> {
        self.publish_event("task_completed", &self.subjects.task_completed, event)
            .await
    }

    async fn publish_task_failed(&self, event: &TaskFailedEvent) -> Result<(), DomainError> {
        self.publish_event("task_failed", &self.subjects.task_failed, event)
            .await
    }

    async fn publish_deliberation_completed(
        &self,
        event: &DeliberationCompletedEvent,
    ) -> Result<(), DomainError> {
        self.publish_event(
            "deliberation_completed",
            &self.subjects.deliberation_completed,
            event,
        )
        .await
    }

    async fn publish_phase_changed(&self, event: &PhaseChangedEvent) -> Result<(), DomainError> {
        self.publish_event("phase_changed", &self.subjects.phase_changed, event)
            .await
    }
}
