use made_core::entities::CeremonyInstance;
use made_core::value_objects::{EventId, TraceId};

/// A folded instance together with the audit identity of its stream head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CeremonyInstanceRead {
    instance: CeremonyInstance,
    trace_id: Option<TraceId>,
    correlation_id: Option<EventId>,
    causation_id: Option<EventId>,
}

impl CeremonyInstanceRead {
    #[must_use]
    pub fn new(
        instance: CeremonyInstance,
        trace_id: Option<TraceId>,
        correlation_id: Option<EventId>,
        causation_id: Option<EventId>,
    ) -> Self {
        Self {
            instance,
            trace_id,
            correlation_id,
            causation_id,
        }
    }

    #[must_use]
    pub fn instance(&self) -> &CeremonyInstance {
        &self.instance
    }

    #[must_use]
    pub fn into_instance(self) -> CeremonyInstance {
        self.instance
    }

    #[must_use]
    pub fn trace_id(&self) -> Option<&TraceId> {
        self.trace_id.as_ref()
    }

    #[must_use]
    pub fn correlation_id(&self) -> Option<&EventId> {
        self.correlation_id.as_ref()
    }

    #[must_use]
    pub fn causation_id(&self) -> Option<&EventId> {
        self.causation_id.as_ref()
    }
}
