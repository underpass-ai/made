use serde::{Deserialize, Serialize};

use crate::events::EventEnvelope;
use crate::value_objects::{Attributes, CouncilContractId, EventId, OutputContractId};

/// Causality and contract metadata that travels with a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TaskMetadata {
    source_event_id: Option<EventId>,
    causation_id: Option<EventId>,
    correlation_id: Option<EventId>,
    council_contract_id: Option<CouncilContractId>,
    output_contract_id: Option<OutputContractId>,
    execution_profile: Attributes,
}

impl TaskMetadata {
    #[must_use]
    pub fn new(
        source_event_id: Option<EventId>,
        causation_id: Option<EventId>,
        correlation_id: Option<EventId>,
        council_contract_id: Option<CouncilContractId>,
        output_contract_id: Option<OutputContractId>,
        execution_profile: Attributes,
    ) -> Self {
        Self {
            source_event_id,
            causation_id,
            correlation_id,
            council_contract_id,
            output_contract_id,
            execution_profile,
        }
    }

    #[must_use]
    pub fn from_trigger_envelope(envelope: &EventEnvelope) -> Self {
        Self::default().with_trigger_envelope(envelope)
    }

    #[must_use]
    pub fn with_trigger_envelope(mut self, envelope: &EventEnvelope) -> Self {
        if self.source_event_id.is_none() {
            self.source_event_id = Some(envelope.event_id().clone());
        }
        if self.causation_id.is_none() {
            self.causation_id = envelope
                .causation_id()
                .cloned()
                .or_else(|| Some(envelope.event_id().clone()));
        }
        if self.correlation_id.is_none() {
            self.correlation_id = envelope
                .correlation_id()
                .cloned()
                .or_else(|| Some(envelope.event_id().clone()));
        }
        self
    }

    #[must_use]
    pub fn source_event_id(&self) -> Option<&EventId> {
        self.source_event_id.as_ref()
    }

    #[must_use]
    pub fn causation_id(&self) -> Option<&EventId> {
        self.causation_id.as_ref()
    }

    #[must_use]
    pub fn correlation_id(&self) -> Option<&EventId> {
        self.correlation_id.as_ref()
    }

    #[must_use]
    pub fn council_contract_id(&self) -> Option<&CouncilContractId> {
        self.council_contract_id.as_ref()
    }

    #[must_use]
    pub fn output_contract_id(&self) -> Option<&OutputContractId> {
        self.output_contract_id.as_ref()
    }

    #[must_use]
    pub fn execution_profile(&self) -> &Attributes {
        &self.execution_profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn derives_causality_from_trigger_envelope() {
        let envelope = EventEnvelope::new_with_causation(
            EventId::new("trigger-1").unwrap(),
            datetime!(2026-04-15 12:00:00 UTC),
            "pir",
            Some(EventId::new("corr-1").unwrap()),
            Some(EventId::new("cause-1").unwrap()),
        )
        .unwrap();

        let metadata = TaskMetadata::from_trigger_envelope(&envelope);

        assert_eq!(metadata.source_event_id().unwrap().as_str(), "trigger-1");
        assert_eq!(metadata.causation_id().unwrap().as_str(), "cause-1");
        assert_eq!(metadata.correlation_id().unwrap().as_str(), "corr-1");
    }
}
