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
}

impl Table {
    pub(crate) const fn key_shape(self) -> KeyShape {
        match self {
            Table::Ceremonies | Table::LegacyStateMigrations => KeyShape::Str,
            Table::Journal | Table::Outbox | Table::Publications => KeyShape::Bytes,
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
        })
    }
}
