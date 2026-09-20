use std::fmt;

use super::KeyShape;

/// The tables an embedded ceremony store consists of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Table {
    /// Sessions as a pre-stream store held them, read-only since the
    /// migration command exists: provenance, not state.
    Ceremonies,
    /// Payload-less receipts a pre-stream store wrote beside them,
    /// read-only for the same reason.
    Journal,
    Publications,
    /// Sealed records, one row per `(ceremony_id, sequence)`.
    Events,
    /// The order every stream shares: global position to events-table key.
    EventLog,
    /// Folded state, one row per `(ceremony_id, version)`.
    Snapshots,
    /// Authoritative stream identities, one row per ceremony id.
    StreamIndex,
    /// Store-wide counters the seam has no primitive for, such as the
    /// last global position.
    Meta,
    /// Last acknowledged global position and lease state per consumer.
    EventCursors,
    /// Visible poison records skipped by each consumer.
    EventCursorQuarantine,
    /// Idempotent session-memory writes, grouped by memory scope.
    MemoryWrites,
    /// Semantic execution roots keyed by stable operation id.
    ExecutionOperations,
    /// One durable intent per operation and accepted claim fence.
    ExecutionIntents,
    /// Connector-reported ambiguity per operation and producing claim fence.
    ExecutionReconciliationRequirements,
    /// The immutable terminal receipt of each semantic operation.
    ExecutionReceipts,
    Councils,
    CouncilAgents,
    CouncilContracts,
    CouncilDeliberations,
    CouncilStatistics,
    CouncilJournal,
    CouncilJournalIds,
    CouncilJournalCursors,
    /// One row per delivery of one item to one host destination.
    HostDeliveries,
    /// Destination index: which deliveries are addressed where.
    HostDeliveryTargets,
    /// Every integrator ever bound to one scope, one row per scope.
    IntegratorBindings,
}

impl Table {
    pub(crate) const fn key_shape(self) -> KeyShape {
        match self {
            Table::Ceremonies
            | Table::StreamIndex
            | Table::Meta
            | Table::EventCursors
            | Table::ExecutionOperations
            | Table::ExecutionReceipts
            | Table::Councils
            | Table::CouncilAgents
            | Table::CouncilContracts
            | Table::CouncilDeliberations
            | Table::CouncilStatistics
            | Table::CouncilJournalIds
            | Table::CouncilJournalCursors
            | Table::HostDeliveries
            | Table::HostDeliveryTargets
            | Table::IntegratorBindings => KeyShape::Str,
            Table::Journal
            | Table::Publications
            | Table::Events
            | Table::EventLog
            | Table::Snapshots
            | Table::EventCursorQuarantine
            | Table::MemoryWrites
            | Table::ExecutionIntents
            | Table::ExecutionReconciliationRequirements
            | Table::CouncilJournal => KeyShape::Bytes,
        }
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Table::Ceremonies => "ceremony_instances",
            Table::Journal => "audit_journal",
            Table::Publications => "published_definitions",
            Table::Events => "ceremony_events",
            Table::EventLog => "ceremony_event_log",
            Table::Snapshots => "ceremony_snapshots",
            Table::StreamIndex => "ceremony_stream_index",
            Table::Meta => "store_meta",
            Table::EventCursors => "ceremony_event_cursors",
            Table::EventCursorQuarantine => "ceremony_event_cursor_quarantine",
            Table::MemoryWrites => "session_memory_writes",
            Table::ExecutionOperations => "execution_operations",
            Table::ExecutionIntents => "execution_intents",
            Table::ExecutionReconciliationRequirements => "execution_reconciliation_requirements",
            Table::ExecutionReceipts => "execution_receipts",
            Table::Councils => "council_registry",
            Table::CouncilAgents => "council_agents",
            Table::CouncilContracts => "council_contracts",
            Table::CouncilDeliberations => "council_deliberations",
            Table::CouncilStatistics => "council_statistics",
            Table::CouncilJournal => "council_journal",
            Table::CouncilJournalIds => "council_journal_ids",
            Table::CouncilJournalCursors => "council_journal_cursors",
            Table::HostDeliveries => "host_deliveries",
            Table::HostDeliveryTargets => "host_delivery_targets",
            Table::IntegratorBindings => "integrator_bindings",
        })
    }
}
