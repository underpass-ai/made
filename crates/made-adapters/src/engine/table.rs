use std::fmt;

use super::KeyShape;

/// The tables an embedded ceremony store consists of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Table {
    Ceremonies,
    Journal,
    Outbox,
    Publications,
    LegacyStateMigrations,
    /// Sealed records, one row per `(ceremony_id, sequence)`.
    Events,
    /// The order every stream shares: global position to events-table key.
    EventLog,
    /// Folded state, one row per `(ceremony_id, version)`.
    Snapshots,
    /// Store-wide counters the seam has no primitive for, such as the
    /// last global position.
    Meta,
}

impl Table {
    pub(crate) const fn key_shape(self) -> KeyShape {
        match self {
            Table::Ceremonies | Table::LegacyStateMigrations | Table::Meta => KeyShape::Str,
            Table::Journal
            | Table::Outbox
            | Table::Publications
            | Table::Events
            | Table::EventLog
            | Table::Snapshots => KeyShape::Bytes,
        }
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Table::Ceremonies => "ceremony_instances",
            Table::Journal => "audit_journal",
            Table::Outbox => "outbox",
            Table::Publications => "published_definitions",
            Table::LegacyStateMigrations => "state_migrations",
            Table::Events => "ceremony_events",
            Table::EventLog => "ceremony_event_log",
            Table::Snapshots => "ceremony_snapshots",
            Table::Meta => "store_meta",
        })
    }
}
