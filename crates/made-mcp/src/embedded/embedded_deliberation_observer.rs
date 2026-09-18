use async_trait::async_trait;
use made_core::entities::DeliberationPhase;
use made_core::ports::DeliberationObserverPort;
use made_core::value_objects::TaskId;
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::sync::mpsc;

/// Call-scoped observer used while one MCP request drains a deliberation.
pub(super) struct EmbeddedDeliberationObserver {
    sink: mpsc::Sender<Value>,
}

impl EmbeddedDeliberationObserver {
    pub(super) fn new(sink: mpsc::Sender<Value>) -> Self {
        Self { sink }
    }
}

impl std::fmt::Debug for EmbeddedDeliberationObserver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EmbeddedDeliberationObserver")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl DeliberationObserverPort for EmbeddedDeliberationObserver {
    async fn on_phase_changed(
        &self,
        task_id: &TaskId,
        phase: DeliberationPhase,
        emitted_at: OffsetDateTime,
    ) {
        let phase = match phase {
            DeliberationPhase::Proposing => "DELIBERATION_PHASE_PROPOSING",
            DeliberationPhase::Revising => "DELIBERATION_PHASE_REVISING",
            DeliberationPhase::Validating => "DELIBERATION_PHASE_VALIDATING",
            DeliberationPhase::Scoring => "DELIBERATION_PHASE_SCORING",
            DeliberationPhase::Completed => "DELIBERATION_PHASE_COMPLETED",
        };
        let frame = json!({
            "task_id": task_id.as_str(),
            "phase": phase,
            "emitted_at": emitted_at.format(&Rfc3339).unwrap_or_default(),
            "payload": Value::Null,
        });
        let _ = self.sink.send(frame).await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;

    fn task_id() -> TaskId {
        TaskId::new("observer-test").unwrap()
    }

    #[tokio::test]
    async fn a_saturated_sink_applies_backpressure_without_losing_the_next_frame() {
        let (sender, mut receiver) = mpsc::channel(1);
        let observer = Arc::new(EmbeddedDeliberationObserver::new(sender));
        observer
            .on_phase_changed(
                &task_id(),
                DeliberationPhase::Proposing,
                OffsetDateTime::UNIX_EPOCH,
            )
            .await;

        let blocked = {
            let observer = observer.clone();
            tokio::spawn(async move {
                observer
                    .on_phase_changed(
                        &task_id(),
                        DeliberationPhase::Revising,
                        OffsetDateTime::UNIX_EPOCH,
                    )
                    .await;
            })
        };
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(
            !blocked.is_finished(),
            "the full channel must apply backpressure"
        );

        assert_eq!(
            receiver.recv().await.unwrap()["phase"],
            "DELIBERATION_PHASE_PROPOSING"
        );
        blocked.await.unwrap();
        assert_eq!(
            receiver.recv().await.unwrap()["phase"],
            "DELIBERATION_PHASE_REVISING"
        );
    }

    #[tokio::test]
    async fn a_closed_sink_finishes_without_waiting_for_a_receiver() {
        let (sender, receiver) = mpsc::channel(1);
        drop(receiver);
        let observer = EmbeddedDeliberationObserver::new(sender);
        tokio::time::timeout(
            Duration::from_millis(100),
            observer.on_phase_changed(
                &task_id(),
                DeliberationPhase::Completed,
                OffsetDateTime::UNIX_EPOCH,
            ),
        )
        .await
        .expect("a dropped MCP request closes the observer immediately");
    }
}
