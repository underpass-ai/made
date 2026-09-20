//! What a builder shows of itself.
//!
//! Which ports a host handed in, never the ports themselves: a trait
//! object has no useful debug shape, and a builder that printed its
//! adapters would put whatever they hold into a log.

use std::fmt;

use crate::EmbeddedMadeBuilder;

impl fmt::Debug for EmbeddedMadeBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EmbeddedMadeBuilder")
            .field("has_definition_repository", &self.definitions.is_some())
            .field("has_ceremony_store", &self.events.is_some())
            .field(
                "has_ceremony_search_cursors",
                &self.ceremony_search_cursors.is_some(),
            )
            .field("has_authorization", &self.authorization.is_some())
            .field("has_event_cursor", &self.cursors.is_some())
            .field("has_event_subscriber", &self.subscriber.is_some())
            .field("has_event_transport", &self.event_transport.is_some())
            .field("has_step_handler", &self.step_handler.is_some())
            .field("has_evidence_source", &self.evidence_source.is_some())
            .field("has_clock", &self.clock.is_some())
            .field("has_metrics", &self.metrics.is_some())
            .field("has_metrics_snapshot", &self.metrics_snapshot.is_some())
            .field("has_statistics", &self.statistics.is_some())
            .field("has_memory", &self.memory.is_some())
            .field("has_council_registry", &self.council_registry.is_some())
            .field("has_agent_registry", &self.agent_registry.is_some())
            .field("has_agent_factory", &self.agent_factory.is_some())
            .field(
                "has_execution_receipt_store",
                &self.execution_receipts.is_some(),
            )
            .field("has_budget_ledger_store", &self.budget_ledger.is_some())
            .finish()
    }
}
