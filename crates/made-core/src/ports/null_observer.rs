use async_trait::async_trait;
use time::OffsetDateTime;

use crate::entities::DeliberationPhase;
use crate::ports::DeliberationObserverPort;
use crate::value_objects::TaskId;

/// No-op deliberation observer for callers without a live stream.
#[derive(Debug, Default, Clone)]
pub struct NullObserver;

#[async_trait]
impl DeliberationObserverPort for NullObserver {
    async fn on_phase_changed(
        &self,
        _task_id: &TaskId,
        _phase: DeliberationPhase,
        _emitted_at: OffsetDateTime,
    ) {
    }
}
